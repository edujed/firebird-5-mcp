//! Row-limit injection: `FIRST {max} SKIP {skip}` clause for SELECT statements.

/// Returns `true` when a `FIRST`/`ROWS` clause can be appended to the
/// statement. Only `SELECT` and `WITH` tolerate a row-limit clause in
/// Firebird; appending it to `EXECUTE` forms is a syntax error.
#[must_use]
pub fn accepts_row_limit(sql: &str) -> bool {
    let trimmed = sql.trim_start();
    let first = trimmed.split_whitespace().next().unwrap_or("");
    matches!(first.to_ascii_uppercase().as_str(), "SELECT" | "WITH")
}

/// Default rows returned by a single query call when the caller does not
/// specify `max_rows`. Kept small so a bare `SELECT *` does not flood the
/// LLM context window.
pub const DEFAULT_ROWS: u32 = 100;

/// Hard cap on rows per query call. Even if the caller asks for more, the
/// injected `FIRST` clause never exceeds this value.
pub const MAX_ROWS: u32 = 1000;

/// Injects a Firebird `FIRST {max} SKIP {skip}` clause into a SELECT
/// statement so the result set is bounded. The clause is inserted
/// immediately after the `SELECT` keyword (or after `WITH ... AS (...)`
/// for CTEs), which is where Firebird's grammar requires it.
///
/// Statements that already contain a row-limiting clause (`FIRST`,
/// `ROWS`, `FETCH FIRST`) are returned unchanged so the user-supplied
/// cap wins.
#[must_use]
pub fn inject_row_limit(sql: &str, max: u32, skip: u32) -> String {
    let max = max.min(MAX_ROWS);
    let trimmed = sql.trim_end().trim_end_matches(';').trim_end();

    // Check if the statement already has a row-limiting clause.
    let upper = trimmed.to_uppercase();
    if upper.contains(" FIRST ")
        || upper.contains(" FETCH FIRST")
        || upper.contains(" ROWS ")
        || upper.ends_with(" ROWS")
    {
        return trimmed.to_string();
    }

    // A `FIRST`/`ROWS`/`FETCH FIRST` clause that sits right after the
    // SELECT keyword (no space before it in the raw text, e.g. a
    // single-line statement) must also be detected.
    if let Some(pos) = find_select_insertion_point(&upper) {
        let after = upper[pos..].trim_start();
        if after.starts_with("FIRST")
            || after.starts_with("ROWS")
            || after.starts_with("FETCH FIRST")
        {
            return trimmed.to_string();
        }
    }

    let clause = if skip > 0 {
        format!("FIRST {max} SKIP {skip}")
    } else {
        format!("FIRST {max}")
    };

    // Find the position to insert the clause. For a plain SELECT, insert
    // after the SELECT keyword. For a WITH (CTE), insert after the final
    // SELECT that follows the CTE definition.
    if let Some(pos) = find_select_insertion_point(&upper) {
        // `pos` is the byte index just after the SELECT keyword. The
        // remaining text starts with whitespace; trim it and rejoin with
        // a single space.
        let rest = trimmed[pos..].trim_start();
        format!("{} {} {}", &trimmed[..pos], clause, rest)
    } else {
        // Fallback: prepend (should not happen for SELECT/WITH).
        format!("{clause} {trimmed}")
    }
}

/// Finds the byte offset just after the `SELECT` keyword where a `FIRST`
/// clause should be inserted. For `SELECT ...`, this is right after
/// `SELECT`. For `WITH cte AS (...) SELECT ...`, this is after the final
/// `SELECT` that follows the CTE body.
fn find_select_insertion_point(upper: &str) -> Option<usize> {
    // Find the last top-level SELECT (not inside parentheses).
    let bytes = upper.as_bytes();
    let mut depth = 0i32;
    let mut last_select_pos = None;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b'S' if depth == 0 => {
                // Check if "SELECT" starts here.
                if upper[i..].starts_with("SELECT") {
                    // Ensure it's a whole word (not e.g. "SELECTED").
                    let after = i + 6;
                    if after >= bytes.len()
                        || !bytes[after].is_ascii_alphanumeric() && bytes[after] != b'_'
                    {
                        last_select_pos = Some(after);
                    }
                }
            }
            _ => {}
        }
        i += 1;
    }
    last_select_pos
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injects_first_after_select() {
        let sql = "SELECT * FROM T";
        let result = inject_row_limit(sql, 100, 0);
        assert_eq!(result, "SELECT FIRST 100 * FROM T");
    }

    #[test]
    fn injects_first_skip_after_select() {
        let sql = "SELECT * FROM T";
        let result = inject_row_limit(sql, 100, 200);
        assert_eq!(result, "SELECT FIRST 100 SKIP 200 * FROM T");
    }

    #[test]
    fn leaves_existing_first_alone() {
        let sql = "SELECT FIRST 10 * FROM T";
        let result = inject_row_limit(sql, 100, 0);
        assert_eq!(result, "SELECT FIRST 10 * FROM T");
    }

    #[test]
    fn leaves_rows_clause_alone() {
        let sql = "SELECT * FROM T ROWS 1 TO 50";
        let result = inject_row_limit(sql, 100, 0);
        assert_eq!(result, "SELECT * FROM T ROWS 1 TO 50");
    }

    #[test]
    fn caps_at_max_rows() {
        let sql = "SELECT * FROM T";
        let result = inject_row_limit(sql, 5000, 0);
        assert!(result.contains("FIRST 1000"));
    }

    #[test]
    fn default_is_100() {
        assert_eq!(DEFAULT_ROWS, 100);
        assert_eq!(MAX_ROWS, 1000);
    }

    #[test]
    fn strips_trailing_semicolon() {
        let sql = "SELECT * FROM T;";
        let result = inject_row_limit(sql, 100, 0);
        assert_eq!(result, "SELECT FIRST 100 * FROM T");
    }

    #[test]
    fn injects_after_cte_select() {
        let sql = "WITH cte AS (SELECT 1 AS x) SELECT * FROM cte";
        let result = inject_row_limit(sql, 50, 0);
        assert!(result.contains("SELECT FIRST 50 * FROM cte"));
    }

    #[test]
    fn handles_where_clause() {
        let sql = "SELECT * FROM T WHERE id > 10";
        let result = inject_row_limit(sql, 10, 0);
        assert_eq!(result, "SELECT FIRST 10 * FROM T WHERE id > 10");
    }

    #[test]
    fn accepts_row_limit_select() {
        assert!(accepts_row_limit("SELECT * FROM T"));
        assert!(accepts_row_limit("  select * from t"));
        assert!(accepts_row_limit(
            "WITH cte AS (SELECT 1) SELECT * FROM cte"
        ));
    }

    #[test]
    fn rejects_row_limit_non_select() {
        assert!(!accepts_row_limit("INSERT INTO T VALUES (1)"));
        assert!(!accepts_row_limit("EXECUTE BLOCK AS BEGIN END"));
        assert!(!accepts_row_limit("CREATE TABLE T (ID INT)"));
    }
}
