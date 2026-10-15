//! Code generation for development entities/structs from Firebird table schemas.
//!
//! Supports: Java (JPA + Lombok), Go (sqlx), Rust (sqlx), TypeScript.

use crate::schema::{ColumnInfo, ForeignKeyInfo, TableSchema};

/// Language target for code generation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Language {
    Java,
    Go,
    Rust,
    TypeScript,
    Protobuf,
    OpenApi,
    Lazarus,
    Svelte,
}

impl Language {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "java" => Some(Self::Java),
            "go" => Some(Self::Go),
            "rust" => Some(Self::Rust),
            "typescript" | "ts" => Some(Self::TypeScript),
            "protobuf" | "proto" => Some(Self::Protobuf),
            "openapi" | "open_api" | "open-api" => Some(Self::OpenApi),
            "lazarus" | "delphi" => Some(Self::Lazarus),
            "svelte" => Some(Self::Svelte),
            _ => None,
        }
    }
}

/// Options for code generation.
#[derive(Debug, Clone)]
pub struct GenOptions {
    pub language: Language,
    pub include_fks: bool,
}

/// Generates code for one or more tables.
pub fn generate(schemas: &[TableSchema], fks: &[ForeignKeyInfo], opts: &GenOptions) -> String {
    let mut out = String::new();
    for schema in schemas {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        let table_fks: Vec<&ForeignKeyInfo> = fks
            .iter()
            .filter(|fk| fk.table.eq_ignore_ascii_case(&schema.table))
            .collect();
        match opts.language {
            Language::Java => out.push_str(&gen_java(schema, &table_fks, opts)),
            Language::Go => out.push_str(&gen_go(schema, &table_fks, opts)),
            Language::Rust => out.push_str(&gen_rust(schema, &table_fks, opts)),
            Language::TypeScript => out.push_str(&gen_ts(schema, &table_fks, opts)),
            Language::Protobuf => out.push_str(&gen_proto(schema, &table_fks, opts)),
            Language::OpenApi => out.push_str(&gen_openapi(schema, &table_fks, opts)),
            Language::Lazarus => {
                // Lazarus is handled separately in the tool handler (generates .lfm files)
                out.push_str(&format!(
                    "// Lazarus form for table {} (use firebird_generate_dev with language=lazarus)\n",
                    schema.table
                ));
            }
            Language::Svelte => {
                // Svelte is handled separately in the tool handler (generates .svelte files)
                out.push_str(&format!(
                    "// Svelte page for table {} (use firebird_generate_dev with language=svelte)\n",
                    schema.table
                ));
            }
        }
    }
    out
}

// --- Naming helpers ---

/// AUT_USUARIO → AutUsuario, PESSOA_ID → PessoaId
pub fn to_pascal_case(s: &str) -> String {
    let camel = to_camel_case(s);
    if camel.is_empty() {
        return String::new();
    }
    let mut chars = camel.chars();
    let first = chars.next().unwrap().to_uppercase().to_string();
    first + chars.as_str()
}

/// PESSOA_ID → pessoa_id, SENHA_HASH → senha_hash, ID → id
pub fn to_snake_case(s: &str) -> String {
    s.split('_')
        .filter(|p| !p.is_empty())
        .map(|p| p.to_lowercase())
        .collect::<Vec<_>>()
        .join("_")
}

/// EMAIL → email, SENHA_HASH → senhaHash, ID → id, PESSOA_ID → pessoaId
pub fn to_camel_case(s: &str) -> String {
    let parts: Vec<&str> = s.split('_').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return String::new();
    }
    let mut result = String::new();
    for (i, part) in parts.iter().enumerate() {
        if i == 0 {
            // First part: all lowercase (handles ID → id, EMAIL → email)
            result.push_str(&part.to_lowercase());
        } else {
            // Subsequent parts: capitalize first letter, rest lowercase
            let mut c = part.chars();
            let first = c
                .next()
                .map(|f| f.to_uppercase().to_string())
                .unwrap_or_default();
            let rest = c.as_str().to_lowercase();
            result.push_str(&first);
            result.push_str(&rest);
        }
    }
    result
}

// --- Type mapping ---

/// Resolves the effective type for a column, considering BLOB sub-type.
struct ResolvedType {
    base: i64,
    sub_type: i64,
    size: i64,
    precision: i64,
    scale: i64,
}

fn resolve(col: &ColumnInfo) -> ResolvedType {
    ResolvedType {
        base: col.type_num,
        sub_type: col.sub_type,
        size: col.size,
        precision: col.precision,
        scale: col.scale,
    }
}

fn is_blob(r: &ResolvedType) -> bool {
    r.base == 45 || r.base == 261
}

fn is_blob_text(r: &ResolvedType) -> bool {
    is_blob(r) && r.sub_type == 1
}

fn is_numeric(r: &ResolvedType) -> bool {
    (r.base == 16 || r.base == 43) && (r.sub_type == 1 || r.sub_type == 2)
}

fn is_decimal(r: &ResolvedType) -> bool {
    r.base == 40
}

fn is_float(r: &ResolvedType) -> bool {
    r.base == 11 || r.base == 27
}

fn is_integer(r: &ResolvedType) -> bool {
    r.base == 7 || r.base == 8
}

fn is_char(r: &ResolvedType) -> bool {
    r.base == 14 || r.base == 20 || r.base == 37 || r.base == 48
}

fn is_date(r: &ResolvedType) -> bool {
    r.base == 9 || r.base == 12
}

fn is_time(r: &ResolvedType) -> bool {
    r.base == 10
}

fn is_timestamp(r: &ResolvedType) -> bool {
    r.base == 29 || r.base == 35
}

fn is_boolean(r: &ResolvedType) -> bool {
    r.base == 23 || r.base == 49
}

fn is_nullable(col: &ColumnInfo) -> bool {
    col.nullable.map(|n| n == 1).unwrap_or(false)
}

// --- Java (JPA + Lombok) ---

fn java_type(r: &ResolvedType) -> String {
    if is_blob_text(r) {
        return "String".to_string();
    }
    if is_blob(r) {
        return "byte[]".to_string();
    }
    if is_numeric(r) || is_decimal(r) {
        return "BigDecimal".to_string();
    }
    if is_float(r) {
        return "Double".to_string();
    }
    if is_integer(r) {
        return if r.base == 7 {
            "Short".to_string()
        } else {
            "Integer".to_string()
        };
    }
    if is_char(r) {
        return "String".to_string();
    }
    if is_date(r) {
        return "LocalDate".to_string();
    }
    if is_time(r) {
        return "LocalTime".to_string();
    }
    if is_timestamp(r) {
        return "LocalDateTime".to_string();
    }
    if is_boolean(r) {
        return "Boolean".to_string();
    }
    "Object".to_string()
}

fn java_imports(schema: &TableSchema) -> Vec<&'static str> {
    let mut imports: Vec<&'static str> = Vec::new();
    imports.push("jakarta.persistence.*");
    imports.push("lombok.*");
    for col in &schema.columns {
        let r = resolve(col);
        if is_numeric(&r) || is_decimal(&r) {
            imports.push("java.math.BigDecimal");
        }
        if is_date(&r) {
            imports.push("java.time.LocalDate");
        }
        if is_time(&r) {
            imports.push("java.time.LocalTime");
        }
        if is_timestamp(&r) {
            imports.push("java.time.LocalDateTime");
        }
    }
    imports.sort();
    imports.dedup();
    imports
}

fn gen_java(schema: &TableSchema, fks: &[&ForeignKeyInfo], opts: &GenOptions) -> String {
    let class_name = to_pascal_case(&schema.table);
    let mut b = String::new();

    // Imports
    for imp in java_imports(schema) {
        b.push_str(&format!("import {}\n", imp));
    }
    b.push('\n');

    // Class annotations
    b.push_str("@Entity\n");
    b.push_str(&format!("@Table(name = \"{}\")\n", schema.table));
    b.push_str("@Data\n");
    b.push_str("@NoArgsConstructor\n");
    b.push_str("@AllArgsConstructor\n");
    b.push_str("@Builder\n");
    b.push_str(&format!("public class {} {{\n\n", class_name));

    // PK columns
    let pk_cols: Vec<String> = schema
        .indexes
        .iter()
        .filter(|i| i.unique && i.name.starts_with("PK_"))
        .flat_map(|i| i.columns.split(',').map(|c| c.trim().to_string()))
        .collect();

    for col in &schema.columns {
        let r = resolve(col);
        let field_name = to_camel_case(&col.name);
        let java_type = java_type(&r);
        let nullable = is_nullable(col);

        // Check if this column is a FK and include_fks is on
        if opts.include_fks {
            if let Some(fk) = fks.iter().find(|fk| fk.column == col.name) {
                let entity_name = to_pascal_case(&fk.ref_table);
                b.push_str("    @ManyToOne\n");
                b.push_str(&format!("    @JoinColumn(name = \"{}\")\n", col.name));
                b.push_str(&format!(
                    "    private {} {}{};\n\n",
                    entity_name,
                    to_camel_case(&fk.column),
                    if nullable { "" } else { "" }
                ));
                continue;
            }
        }

        // Regular column
        if pk_cols.contains(&col.name) {
            b.push_str("    @Id\n");
        }
        if is_blob(&r) {
            b.push_str("    @Lob\n");
        }
        if is_char(&r) && r.size > 0 {
            let char_size = if r.base == 48 { r.size / 2 } else { r.size / 4 };
            b.push_str(&format!(
                "    @Column(name = \"{}\", length = {})",
                col.name, char_size
            ));
        } else if is_numeric(&r) || is_decimal(&r) {
            let scale_abs = r.scale.abs();
            if r.precision > 0 && scale_abs > 0 {
                b.push_str(&format!(
                    "    @Column(name = \"{}\", precision = {}, scale = {})",
                    col.name, r.precision, scale_abs
                ));
            } else if r.precision > 0 {
                b.push_str(&format!(
                    "    @Column(name = \"{}\", precision = {})",
                    col.name, r.precision
                ));
            } else {
                b.push_str(&format!("    @Column(name = \"{}\")", col.name));
            }
        } else {
            b.push_str(&format!("    @Column(name = \"{}\")", col.name));
        }
        b.push_str("\n");
        b.push_str(&format!(
            "    private {} {}{};\n\n",
            java_type,
            field_name,
            if nullable { "" } else { "" }
        ));
    }

    b.push_str("}");
    b
}

// --- Go (sqlx) ---

fn go_type(r: &ResolvedType) -> String {
    if is_blob_text(r) {
        return "String".to_string();
    }
    if is_blob(r) {
        return "Vec<u8>".to_string();
    }
    if is_numeric(r) || is_decimal(r) {
        return "Decimal".to_string();
    }
    if is_float(r) {
        return "f64".to_string();
    }
    if is_integer(r) {
        return if r.base == 7 {
            "i16".to_string()
        } else {
            "i32".to_string()
        };
    }
    if is_char(r) {
        return "String".to_string();
    }
    if is_date(r) {
        return "NaiveDate".to_string();
    }
    if is_time(r) {
        return "NaiveTime".to_string();
    }
    if is_timestamp(r) {
        return "NaiveDateTime".to_string();
    }
    if is_boolean(r) {
        return "bool".to_string();
    }
    "interface{}".to_string()
}

fn go_null_type(t: &str) -> String {
    match t {
        "i16" => "sql.NullInt16".to_string(),
        "i32" => "sql.NullInt32".to_string(),
        "f64" => "sql.NullFloat64".to_string(),
        "String" => "sql.NullString".to_string(),
        "bool" => "sql.NullBool".to_string(),
        "NaiveDate" => "*time.Time".to_string(),
        "NaiveTime" => "*time.Time".to_string(),
        "NaiveDateTime" => "*time.Time".to_string(),
        "Decimal" => "*Decimal".to_string(),
        _ => format!("*{t}"),
    }
}

fn gen_go(schema: &TableSchema, fks: &[&ForeignKeyInfo], opts: &GenOptions) -> String {
    let struct_name = to_pascal_case(&schema.table);
    let mut b = String::new();

    b.push_str(&format!(
        "// {} maps to table {}\n",
        struct_name, schema.table
    ));
    b.push_str(&format!("type {} struct {{\n", struct_name));

    for col in &schema.columns {
        let r = resolve(col);
        let field_name = to_pascal_case(&col.name);
        let base_type = go_type(&r);
        let nullable = is_nullable(col);
        let json_name = to_camel_case(&col.name);

        let type_str = if nullable {
            go_null_type(&base_type)
        } else {
            base_type
        };

        let mut comment = String::new();
        if opts.include_fks {
            if let Some(fk) = fks.iter().find(|fk| fk.column == col.name) {
                comment = format!(" // FK → {}.{}", fk.ref_table, fk.ref_column);
            }
        }

        b.push_str(&format!(
            "    {:<20} {:<20} `db:\"{}\" json:\"{}\"`{}\n",
            field_name, type_str, col.name, json_name, comment
        ));
    }

    b.push_str("}");
    b
}

// --- Rust (sqlx) ---

fn rust_type(r: &ResolvedType) -> String {
    if is_blob_text(r) {
        return "String".to_string();
    }
    if is_blob(r) {
        return "Vec<u8>".to_string();
    }
    if is_numeric(r) || is_decimal(r) {
        return "Decimal".to_string();
    }
    if is_float(r) {
        return "f64".to_string();
    }
    if is_integer(r) {
        return if r.base == 7 {
            "i16".to_string()
        } else {
            "i32".to_string()
        };
    }
    if is_char(r) {
        return "String".to_string();
    }
    if is_date(r) {
        return "NaiveDate".to_string();
    }
    if is_time(r) {
        return "NaiveTime".to_string();
    }
    if is_timestamp(r) {
        return "NaiveDateTime".to_string();
    }
    if is_boolean(r) {
        return "bool".to_string();
    }
    "i32".to_string()
}

fn gen_rust(schema: &TableSchema, fks: &[&ForeignKeyInfo], opts: &GenOptions) -> String {
    let struct_name = to_pascal_case(&schema.table);
    let mut b = String::new();

    b.push_str(&format!(
        "// {} maps to table {}\n",
        struct_name, schema.table
    ));
    b.push_str("#[derive(Debug, Clone, FromRow)]\n");
    b.push_str(&format!("pub struct {} {{\n", struct_name));

    for col in &schema.columns {
        let r = resolve(col);
        let field_name = to_snake_case(&col.name);
        let base_type = rust_type(&r);
        let nullable = is_nullable(col);

        let mut comment = format!("    /// Column: {}", col.name);
        if opts.include_fks {
            if let Some(fk) = fks.iter().find(|fk| fk.column == col.name) {
                comment.push_str(&format!(" — FK → {}.{}", fk.ref_table, fk.ref_column));
            }
        }
        comment.push('\n');

        let type_str = if nullable {
            format!("Option<{}>", base_type)
        } else {
            base_type
        };

        b.push_str(&comment);
        b.push_str(&format!("    pub {}: {},\n", field_name, type_str));
    }

    b.push_str("}\n");

    // Example query with aliases matching field names
    b.push_str("\n// Example query (aliases must match field names for FromRow):\n");
    let aliases: Vec<String> = schema
        .columns
        .iter()
        .map(|col| format!("{} AS {}", col.name, to_snake_case(&col.name)))
        .collect();
    b.push_str(&format!(
        "// SELECT {} FROM {}\n",
        aliases.join(", "),
        schema.table
    ));
    b.push_str(&format!(
        "// let rows: Vec<{}> = conn.query(sql, ({{}}))?;\n",
        struct_name
    ));
    b
}

// --- TypeScript ---

fn ts_type(r: &ResolvedType) -> String {
    if is_blob_text(r) {
        return "string".to_string();
    }
    if is_blob(r) {
        return "string".to_string(); // base64
    }
    if is_numeric(r) || is_decimal(r) {
        return "number".to_string();
    }
    if is_float(r) {
        return "number".to_string();
    }
    if is_integer(r) {
        return "number".to_string();
    }
    if is_char(r) {
        return "string".to_string();
    }
    if is_date(r) || is_time(r) || is_timestamp(r) {
        return "string".to_string(); // ISO format
    }
    if is_boolean(r) {
        return "boolean".to_string();
    }
    "unknown".to_string()
}

fn gen_ts(schema: &TableSchema, fks: &[&ForeignKeyInfo], opts: &GenOptions) -> String {
    let interface_name = to_pascal_case(&schema.table);
    let mut b = String::new();

    b.push_str(&format!("export interface {} {{\n", interface_name));

    for col in &schema.columns {
        let r = resolve(col);
        let field_name = to_camel_case(&col.name);
        let ts_type = ts_type(&r);
        let nullable = is_nullable(col);

        let mut comment = String::new();
        if opts.include_fks {
            if let Some(fk) = fks.iter().find(|fk| fk.column == col.name) {
                comment = format!("    /** FK → {}.{} */\n", fk.ref_table, fk.ref_column);
            }
        }

        let opt = if nullable { "?" } else { "" };
        b.push_str(&comment);
        b.push_str(&format!("    {}{}: {};\n", field_name, opt, ts_type));
    }

    b.push_str("}");
    b
}

// --- Protobuf (Connect-RPC / gRPC) ---

fn proto_type(r: &ResolvedType) -> String {
    if is_blob_text(r) {
        return "string".to_string();
    }
    if is_blob(r) {
        return "bytes".to_string();
    }
    if is_numeric(r) || is_decimal(r) {
        return "string".to_string(); // Use string for precision
    }
    if is_float(r) {
        return "double".to_string();
    }
    if is_integer(r) {
        return "int32".to_string();
    }
    if is_char(r) {
        return "string".to_string();
    }
    if is_date(r) || is_time(r) || is_timestamp(r) {
        return "string".to_string(); // ISO format
    }
    if is_boolean(r) {
        return "bool".to_string();
    }
    "string".to_string()
}

fn gen_proto(schema: &TableSchema, fks: &[&ForeignKeyInfo], opts: &GenOptions) -> String {
    let message_name = to_pascal_case(&schema.table);
    let mut b = String::new();

    b.push_str("syntax = \"proto3\";\n\n");
    b.push_str(&format!("package {};\n\n", schema.table.to_lowercase()));

    // Nullable columns comment
    let nullable_cols: Vec<&str> = schema
        .columns
        .iter()
        .filter(|c| is_nullable(c))
        .map(|c| c.name.as_str())
        .collect();
    if !nullable_cols.is_empty() {
        b.push_str(&format!(
            "// Nullable columns: {}\n",
            nullable_cols.join(", ")
        ));
    }

    b.push_str(&format!("message {} {{\n", message_name));

    for (idx, col) in schema.columns.iter().enumerate() {
        let r = resolve(col);
        let field_name = to_snake_case(&col.name);
        let proto_type = proto_type(&r);
        let field_num = idx + 1; // Protobuf fields start at 1

        let mut comments = Vec::new();
        if let Some(comment) = schema.comments.get(&col.name) {
            comments.push(comment.clone());
        }
        if opts.include_fks {
            if let Some(fk) = fks.iter().find(|fk| fk.column == col.name) {
                comments.push(format!("FK → {}.{}", fk.ref_table, fk.ref_column));
            }
        }
        for c in &comments {
            b.push_str(&format!("    // {}\n", c));
        }

        b.push_str(&format!(
            "    {} {} = {};\n",
            proto_type, field_name, field_num
        ));
    }

    b.push_str("}");
    b
}

// --- OpenAPI 3.2.0 ---

fn openapi_type(r: &ResolvedType) -> (String, Option<String>) {
    // Returns (type, format)
    if is_blob_text(r) {
        return ("string".to_string(), None);
    }
    if is_blob(r) {
        return ("string".to_string(), Some("binary".to_string()));
    }
    if is_numeric(r) || is_decimal(r) {
        return ("number".to_string(), Some("decimal".to_string()));
    }
    if is_float(r) {
        return ("number".to_string(), Some("double".to_string()));
    }
    if is_integer(r) {
        return ("integer".to_string(), Some("int32".to_string()));
    }
    if is_char(r) {
        return ("string".to_string(), None);
    }
    if is_date(r) {
        return ("string".to_string(), Some("date".to_string()));
    }
    if is_time(r) {
        return ("string".to_string(), Some("time".to_string()));
    }
    if is_timestamp(r) {
        return ("string".to_string(), Some("date-time".to_string()));
    }
    if is_boolean(r) {
        return ("boolean".to_string(), None);
    }
    ("string".to_string(), None)
}

fn gen_openapi(schema: &TableSchema, fks: &[&ForeignKeyInfo], opts: &GenOptions) -> String {
    let schema_name = to_pascal_case(&schema.table);
    let mut b = String::new();

    b.push_str(&format!("  {}:\n", schema_name));
    b.push_str("    type: object\n");
    b.push_str(&format!(
        "    description: Schema for table {}\n",
        schema.table
    ));

    // Table comment
    if let Some(table_comment) = schema.comments.get("table") {
        b.push_str(&format!("    description: {}\n", table_comment));
    }

    // Required fields (NOT NULL)
    let required: Vec<String> = schema
        .columns
        .iter()
        .filter(|c| !is_nullable(c))
        .map(|c| to_camel_case(&c.name))
        .collect();
    if !required.is_empty() {
        b.push_str("    required:\n");
        for field in &required {
            b.push_str(&format!("      - {}\n", field));
        }
    }

    b.push_str("    properties:\n");

    for col in &schema.columns {
        let r = resolve(col);
        let field_name = to_camel_case(&col.name);
        let (type_str, format_str) = openapi_type(&r);
        let nullable = is_nullable(col);

        b.push_str(&format!("      {}:\n", field_name));
        b.push_str(&format!("        type: {}\n", type_str));
        if let Some(fmt) = &format_str {
            b.push_str(&format!("        format: {}\n", fmt));
        }
        if nullable {
            b.push_str("        nullable: true\n");
        }

        // Description from comments
        if let Some(comment) = schema.comments.get(&col.name) {
            b.push_str(&format!("        description: {}\n", comment));
        }

        // FK reference
        if opts.include_fks {
            if let Some(fk) = fks.iter().find(|fk| fk.column == col.name) {
                b.push_str(&format!(
                    "        x-foreign-key: {}.{}\n",
                    fk.ref_table, fk.ref_column
                ));
            }
        }
    }

    b
}
