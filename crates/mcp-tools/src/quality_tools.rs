//! Data quality tools: rule-based checks on a single table.
//!
//! `firebird_data_quality` runs configurable rules (nulls, empty_strings,
//! outliers, date_range) against a table and returns findings with severity,
//! counts, and sample values. Output can be Markdown (default) or JSON.

use firebird_5_mcp_core::types::is_valid_identifier;
use firebird_5_mcp_core::{Config, FirebirdClient, query, schema};
use serde_json::{Value, json};

use super::ToolResult;
use super::common::{arg_str, arg_u32, open_client};

/// All supported rule names.
const ALL_RULES: &[&str] = &["nulls", "empty_strings", "outliers", "date_range"];

/// RDB$FIELD_TYPE codes for character types (CHAR, VARCHAR, CSTRING, NVARCHAR).
const CHAR_TYPES: &[i64] = &[14, 20, 37, 40, 48];
/// RDB$FIELD_TYPE codes for numeric types (SMALLINT, INTEGER, FLOAT, DOUBLE,
/// BIGINT, DECFLOAT, INT128, NUMERIC/DECIMAL).
const NUMERIC_TYPES: &[i64] = &[7, 8, 10, 11, 16, 24, 25, 26, 27, 43];
/// RDB$FIELD_TYPE codes for date/time types (DATE, TIME, TIMESTAMP).
const DATE_TYPES: &[i64] = &[9, 12, 13, 29, 35];

/// A single quality finding.
#[derive(Debug, Clone)]
struct Finding {
    rule: &'static str,
    column: String,
    severity: &'static str,
    count: i64,
    samples: Vec<String>,
    /// Extra context for outliers (Q1, Q3, bounds).
    extra: Option<Value>,
}

/// `firebird_data_quality` — run data quality rules on a table.
pub fn handle_data_quality(cfg: &Config, args: &Value) -> Result<ToolResult, String> {
    let table = arg_str(args, "table")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "missing required argument: table".to_string())?;
    let table = table.trim().to_uppercase();
    if !is_valid_identifier(&table) {
        return Err(format!("Invalid table name: {table}"));
    }

    let rules: Vec<String> = args
        .get("rules")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_lowercase()))
                .collect()
        })
        .filter(|v: &Vec<String>| !v.is_empty())
        .unwrap_or_else(|| ALL_RULES.iter().map(|s| s.to_string()).collect());

    for r in &rules {
        if !ALL_RULES.contains(&r.as_str()) {
            return Err(format!(
                "unknown rule '{r}' — valid: {}",
                ALL_RULES.join(", ")
            ));
        }
    }

    let columns: Vec<String> = args
        .get("columns")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_uppercase()))
                .collect()
        })
        .filter(|v: &Vec<String>| !v.is_empty())
        .unwrap_or_default();

    let output_format = arg_str(args, "output_format")
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| "markdown".to_string());
    if output_format != "markdown" && output_format != "json" {
        return Err(format!(
            "invalid output_format '{output_format}' — valid: 'markdown', 'json'"
        ));
    }

    let sample_limit = arg_u32(args, "sample_limit").unwrap_or(5).min(20);

    let mut client = open_client(cfg, args)?;

    // 1. Get column metadata
    let table_schema = schema::describe_table(&mut client, &table).map_err(|e| e.to_string())?;

    // Filter columns if requested
    let cols: Vec<&schema::ColumnInfo> = if columns.is_empty() {
        table_schema.columns.iter().collect()
    } else {
        table_schema
            .columns
            .iter()
            .filter(|c| columns.iter().any(|req| req.eq_ignore_ascii_case(&c.name)))
            .collect()
    };

    if cols.is_empty() {
        return Err(format!(
            "no columns found in table {table} matching the requested filter"
        ));
    }

    // 2. Run rules
    let mut findings: Vec<Finding> = Vec::new();

    for rule in &rules {
        match rule.as_str() {
            "nulls" => run_nulls(&mut client, &table, &cols, sample_limit, &mut findings),
            "empty_strings" => {
                run_empty_strings(&mut client, &table, &cols, sample_limit, &mut findings)
            }
            "outliers" => run_outliers(&mut client, &table, &cols, sample_limit, &mut findings),
            "date_range" => run_date_range(&mut client, &table, &cols, sample_limit, &mut findings),
            _ => {}
        }
    }

    // 3. Build output
    let total_issues = findings.len() as i64;
    let findings_json: Vec<Value> = findings
        .iter()
        .map(|f| {
            let mut obj = json!({
                "rule": f.rule,
                "column": f.column,
                "severity": f.severity,
                "count": f.count,
                "samples": f.samples,
            });
            if let Some(ref extra) = f.extra {
                obj.as_object_mut()
                    .unwrap()
                    .insert("extra".to_string(), extra.clone());
            }
            obj
        })
        .collect();

    let structured = json!({
        "table": table,
        "rules": rules,
        "total_issues": total_issues,
        "findings": findings_json,
    });

    let text = if output_format == "json" {
        serde_json::to_string_pretty(&structured).unwrap_or_default()
    } else {
        format_markdown(&table, &rules, &findings)
    };

    Ok(ToolResult::with_structured(text, structured))
}

// ---------------------------------------------------------------------------
// Rule implementations
// ---------------------------------------------------------------------------

/// Rule: NULLs in NOT NULL columns.
fn run_nulls(
    client: &mut FirebirdClient,
    table: &str,
    cols: &[&schema::ColumnInfo],
    sample_limit: u32,
    findings: &mut Vec<Finding>,
) {
    for col in cols {
        // RDB$NULL_FLAG: 1 = NOT NULL, NULL/0 = nullable
        if !firebird_5_mcp_types::is_not_null(col.nullable) {
            continue;
        }

        let sql = format!(
            "SELECT COUNT(*) FROM \"{table}\" WHERE \"{name}\" IS NULL",
            name = col.name
        );
        let count = match query_count(client, &sql) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "data_quality: nulls rule failed for {table}.{name}: {e}",
                    name = col.name
                );
                continue;
            }
        };

        if count > 0 {
            findings.push(Finding {
                rule: "nulls",
                column: col.name.clone(),
                severity: "error",
                count,
                samples: vec!["NULL".repeat(sample_limit as usize)],
                extra: None,
            });
        }
    }
}

/// Rule: empty strings in character columns.
fn run_empty_strings(
    client: &mut FirebirdClient,
    table: &str,
    cols: &[&schema::ColumnInfo],
    sample_limit: u32,
    findings: &mut Vec<Finding>,
) {
    for col in cols {
        if !CHAR_TYPES.contains(&col.type_num) {
            continue;
        }

        let sql = format!(
            "SELECT COUNT(*) FROM \"{table}\" WHERE \"{name}\" = ''",
            name = col.name
        );
        let count = match query_count(client, &sql) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "data_quality: empty_strings rule failed for {table}.{name}: {e}",
                    name = col.name
                );
                continue;
            }
        };

        if count > 0 {
            findings.push(Finding {
                rule: "empty_strings",
                column: col.name.clone(),
                severity: "warning",
                count,
                samples: vec!["''".repeat(sample_limit as usize)],
                extra: None,
            });
        }
    }
}

/// Rule: IQR-based outliers in numeric columns.
fn run_outliers(
    client: &mut FirebirdClient,
    table: &str,
    cols: &[&schema::ColumnInfo],
    sample_limit: u32,
    findings: &mut Vec<Finding>,
) {
    for col in cols {
        if !NUMERIC_TYPES.contains(&col.type_num) {
            continue;
        }

        // 1. Count non-null values
        let count_sql = format!(
            "SELECT COUNT(*) FROM \"{table}\" WHERE \"{name}\" IS NOT NULL",
            name = col.name
        );
        let n = match query_count(client, &count_sql) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "data_quality: outliers count failed for {table}.{name}: {e}",
                    name = col.name
                );
                continue;
            }
        };
        if n < 4 {
            continue; // not enough data for meaningful quartiles
        }

        // 2. Q1 (25th percentile)
        let q1_skip = (n as f64 * 0.25).floor() as u32;
        let q1_sql = format!(
            "SELECT \"{name}\" FROM \"{table}\" WHERE \"{name}\" IS NOT NULL ORDER BY \"{name}\" FIRST 1 SKIP {skip}",
            name = col.name,
            skip = q1_skip
        );
        let q1 = match query_first_value(client, &q1_sql) {
            Ok(v) => v,
            Err(e) => {
                eprintln!(
                    "data_quality: outliers Q1 failed for {table}.{name}: {e}",
                    name = col.name
                );
                continue;
            }
        };

        // 3. Q3 (75th percentile)
        let q3_skip = (n as f64 * 0.75).floor() as u32;
        let q3_sql = format!(
            "SELECT \"{name}\" FROM \"{table}\" WHERE \"{name}\" IS NOT NULL ORDER BY \"{name}\" FIRST 1 SKIP {skip}",
            name = col.name,
            skip = q3_skip
        );
        let q3 = match query_first_value(client, &q3_sql) {
            Ok(v) => v,
            Err(e) => {
                eprintln!(
                    "data_quality: outliers Q3 failed for {table}.{name}: {e}",
                    name = col.name
                );
                continue;
            }
        };

        // 4. Compute IQR bounds
        let q1_f = value_as_f64(&q1);
        let q3_f = value_as_f64(&q3);
        let (q1_f, q3_f) = match (q1_f, q3_f) {
            (Some(a), Some(b)) => (a, b),
            _ => continue, // can't compute bounds
        };
        let iqr = q3_f - q1_f;
        if iqr <= 0.0 {
            continue; // all values identical, no outliers possible
        }
        let lower = q1_f - 1.5 * iqr;
        let upper = q3_f + 1.5 * iqr;

        // 5. Count outliers
        let outlier_sql = format!(
            "SELECT COUNT(*) FROM \"{table}\" WHERE \"{name}\" < {lower} OR \"{name}\" > {upper}",
            name = col.name,
            lower = format_num(lower),
            upper = format_num(upper)
        );
        let outlier_count = match query_count(client, &outlier_sql) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "data_quality: outliers count failed for {table}.{name}: {e}",
                    name = col.name
                );
                continue;
            }
        };

        if outlier_count > 0 {
            // 6. Sample outlier values
            let sample_sql = format!(
                "SELECT \"{name}\" FROM \"{table}\" WHERE \"{name}\" < {lower} OR \"{name}\" > {upper} FIRST {limit}",
                name = col.name,
                lower = format_num(lower),
                upper = format_num(upper),
                limit = sample_limit
            );
            let samples = query_sample_values(client, &sample_sql);

            findings.push(Finding {
                rule: "outliers",
                column: col.name.clone(),
                severity: "warning",
                count: outlier_count,
                samples,
                extra: Some(json!({
                    "q1": format_num(q1_f),
                    "q3": format_num(q3_f),
                    "iqr": format_num(iqr),
                    "lower_bound": format_num(lower),
                    "upper_bound": format_num(upper),
                })),
            });
        }
    }
}

/// Rule: dates outside the range [1900-01-01, CURRENT_DATE + 1].
fn run_date_range(
    client: &mut FirebirdClient,
    table: &str,
    cols: &[&schema::ColumnInfo],
    sample_limit: u32,
    findings: &mut Vec<Finding>,
) {
    for col in cols {
        if !DATE_TYPES.contains(&col.type_num) {
            continue;
        }

        // TIME columns (type 13) don't have a meaningful date range check
        if col.type_num == 13 {
            continue;
        }

        let sql = format!(
            "SELECT COUNT(*) FROM \"{table}\" WHERE \"{name}\" > CURRENT_DATE + 1 OR \"{name}\" < DATE '1900-01-01'",
            name = col.name
        );
        let count = match query_count(client, &sql) {
            Ok(c) => c,
            Err(e) => {
                eprintln!(
                    "data_quality: date_range rule failed for {table}.{name}: {e}",
                    name = col.name
                );
                continue;
            }
        };

        if count > 0 {
            let sample_sql = format!(
                "SELECT \"{name}\" FROM \"{table}\" WHERE \"{name}\" > CURRENT_DATE + 1 OR \"{name}\" < DATE '1900-01-01' FIRST {limit}",
                name = col.name,
                limit = sample_limit
            );
            let samples = query_sample_values(client, &sample_sql);

            findings.push(Finding {
                rule: "date_range",
                column: col.name.clone(),
                severity: "info",
                count,
                samples,
                extra: None,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Query helpers
// ---------------------------------------------------------------------------

/// Executes a COUNT(*) query and returns the count.
fn query_count(client: &mut FirebirdClient, sql: &str) -> Result<i64, String> {
    let result = query::query(client, sql).map_err(|e| e.to_string())?;
    result
        .rows
        .first()
        .and_then(|row| row.first())
        .and_then(|v| v.as_i64())
        .ok_or_else(|| "unexpected result shape from COUNT(*)".to_string())
}

/// Executes a query and returns the first value of the first row as f64.
fn query_first_value(client: &mut FirebirdClient, sql: &str) -> Result<query::Value, String> {
    let result = query::query(client, sql).map_err(|e| e.to_string())?;
    result
        .rows
        .first()
        .and_then(|row| row.first())
        .cloned()
        .ok_or_else(|| "no rows returned".to_string())
}

/// Executes a query and returns the first column of each row as formatted strings.
fn query_sample_values(client: &mut FirebirdClient, sql: &str) -> Vec<String> {
    let Ok(result) = query::query(client, sql) else {
        return Vec::new();
    };
    result
        .rows
        .iter()
        .filter_map(|row| row.first())
        .map(|v| format_cell_value(v))
        .collect()
}

/// Formats a Value for display in samples.
fn format_cell_value(v: &query::Value) -> String {
    firebird_5_mcp_core::types::format_cell(v)
}

/// Returns the value as f64, parsing text and coercing integers/booleans.
fn value_as_f64(v: &query::Value) -> Option<f64> {
    match v {
        query::Value::Float(f) => Some(*f),
        query::Value::Int(n) => Some(*n as f64),
        query::Value::Bool(b) => Some(f64::from(*b)),
        query::Value::Text(s) => s.parse::<f64>().ok(),
        _ => None,
    }
}

/// Formats a float for SQL interpolation (avoids scientific notation).
fn format_num(f: f64) -> String {
    if f == f.trunc() && f.abs() < 1e15 {
        format!("{}", f as i64)
    } else {
        format!("{f:.4}")
    }
}

// ---------------------------------------------------------------------------
// Output formatting
// ---------------------------------------------------------------------------

/// Formats findings as a Markdown report.
fn format_markdown(table: &str, rules: &[String], findings: &[Finding]) -> String {
    let mut b = String::new();
    b.push_str(&format!("## Data Quality: `{table}`\n\n"));
    b.push_str(&format!("**Rules:** {}\n", rules.join(", ")));
    b.push_str(&format!("**Issues:** {}\n\n", findings.len()));

    for rule in ALL_RULES {
        if !rules.iter().any(|r| r == rule) {
            continue;
        }
        let rule_findings: Vec<&Finding> = findings.iter().filter(|f| f.rule == *rule).collect();

        let (icon, title) = match *rule {
            "nulls" => ("🔴", "nulls"),
            "empty_strings" => ("🟡", "empty_strings"),
            "outliers" => ("🟡", "outliers"),
            "date_range" => ("ℹ️", "date_range"),
            _ => ("", *rule),
        };

        b.push_str(&format!("### {icon} {title}\n\n"));

        if rule_findings.is_empty() {
            b.push_str("No issues.\n\n");
            continue;
        }

        if *rule == "outliers" {
            b.push_str("| Column | Q1 | Q3 | Bounds | Count | Sample |\n");
            b.push_str("|--------|-----|-----|--------|-------|--------|\n");
            for f in &rule_findings {
                let extra = f.extra.as_ref();
                let q1 = extra
                    .and_then(|e| e.get("q1"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("-");
                let q3 = extra
                    .and_then(|e| e.get("q3"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("-");
                let lower = extra
                    .and_then(|e| e.get("lower_bound"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("-");
                let upper = extra
                    .and_then(|e| e.get("upper_bound"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("-");
                let bounds = format!("[{lower}, {upper}]");
                let samples = f.samples.join(", ");
                b.push_str(&format!(
                    "| `{}` | {} | {} | {} | {} | {} |\n",
                    f.column, q1, q3, bounds, f.count, samples
                ));
            }
        } else {
            b.push_str("| Column | Count | Sample |\n");
            b.push_str("|--------|-------|--------|\n");
            for f in &rule_findings {
                let samples = f.samples.join(", ");
                b.push_str(&format!("| `{}` | {} | {} |\n", f.column, f.count, samples));
            }
        }
        b.push('\n');
    }

    b
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(rule: &'static str, column: &str, count: i64) -> Finding {
        Finding {
            rule,
            column: column.to_string(),
            severity: "warning",
            count,
            samples: vec!["123".to_string()],
            extra: None,
        }
    }

    #[test]
    fn test_format_num_integer() {
        assert_eq!(format_num(42.0), "42");
        assert_eq!(format_num(-7.0), "-7");
        assert_eq!(format_num(0.0), "0");
    }

    #[test]
    fn test_format_num_float() {
        assert_eq!(format_num(1.5), "1.5000");
        assert_eq!(format_num(-0.25), "-0.2500");
    }

    #[test]
    fn test_value_as_f64() {
        assert_eq!(value_as_f64(&query::Value::Float(3.14)), Some(3.14));
        assert_eq!(value_as_f64(&query::Value::Int(42)), Some(42.0));
        assert_eq!(value_as_f64(&query::Value::Bool(true)), Some(1.0));
        assert_eq!(value_as_f64(&query::Value::Text("12.5".into())), Some(12.5));
        assert_eq!(value_as_f64(&query::Value::Text("abc".into())), None);
        assert_eq!(value_as_f64(&query::Value::Null), None);
    }

    #[test]
    fn test_format_markdown_no_findings() {
        let md = format_markdown("USERS", &["nulls".to_string()], &[]);
        assert!(md.contains("## Data Quality: `USERS`"));
        assert!(md.contains("**Issues:** 0"));
        assert!(md.contains("No issues."));
    }

    #[test]
    fn test_format_markdown_with_findings() {
        let findings = vec![finding("nulls", "EMAIL", 3)];
        let md = format_markdown("USERS", &["nulls".to_string()], &findings);
        assert!(md.contains("**Issues:** 1"));
        assert!(md.contains("`EMAIL`"));
        assert!(md.contains("| 3 |"));
    }

    #[test]
    fn test_format_markdown_outliers_extra() {
        let mut f = finding("outliers", "SALARY", 5);
        f.extra = Some(json!({
            "q1": "1000",
            "q3": "5000",
            "iqr": "4000",
            "lower_bound": "-5000",
            "upper_bound": "11000",
        }));
        let md = format_markdown("EMPLOYEES", &["outliers".to_string()], &[f]);
        assert!(md.contains("| Q1 | Q3 | Bounds |"));
        assert!(md.contains("[-5000, 11000]"));
        assert!(md.contains("1000"));
        assert!(md.contains("5000"));
    }

    #[test]
    fn test_rule_validation_in_handler() {
        // Unknown rule should be rejected before any DB access.
        let args = json!({"table": "USERS", "rules": ["bogus_rule"]});
        let err = match handle_data_quality(&Config::default_empty(), &args) {
            Err(e) => e,
            Ok(_) => panic!("unknown rule must be rejected"),
        };
        assert!(err.contains("unknown rule"));
        assert!(err.contains("bogus_rule"));
    }

    #[test]
    fn test_output_format_validation() {
        let args = json!({"table": "USERS", "output_format": "yaml"});
        let err = match handle_data_quality(&Config::default_empty(), &args) {
            Err(e) => e,
            Ok(_) => panic!("invalid output_format must be rejected"),
        };
        assert!(err.contains("invalid output_format"));
    }

    #[test]
    fn test_missing_table_arg() {
        let args = json!({});
        let err = match handle_data_quality(&Config::default_empty(), &args) {
            Err(e) => e,
            Ok(_) => panic!("missing table must be rejected"),
        };
        assert!(err.contains("missing required argument: table"));
    }

    #[test]
    fn test_invalid_table_name() {
        let args = json!({"table": "DROP TABLE USERS; --"});
        let err = match handle_data_quality(&Config::default_empty(), &args) {
            Err(e) => e,
            Ok(_) => panic!("invalid table name must be rejected"),
        };
        assert!(err.contains("Invalid table name"));
    }
}
