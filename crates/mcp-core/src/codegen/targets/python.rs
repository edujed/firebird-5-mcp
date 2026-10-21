//! Python target: SQLAlchemy 2.0 ORM models + Pydantic v2 schemas.
//!
//! Each table produces two files:
//! - `<table>.py` (format `python`) — SQLAlchemy 2.0 `DeclarativeBase` model
//!   (`Mapped`/`mapped_column`, `__tablename__`, PK/FK via `ForeignKey`)
//! - `<table>_schemas.py` (format `python-pydantic`) — Pydantic v2
//!   `Base`/`Create`/`Update`/`Read` schemas (all fields `Optional` in
//!   `Update`; `Read` mirrors the model)
//!
//! Type mapping (SQLAlchemy):
//! - `NUMERIC`/`DECIMAL` → `Numeric(precision, scale)` (exact, matches the
//!   vendored driver's decimal-as-text rendering)
//! - `BLOB SUB_TYPE TEXT` → `Text`; `BLOB SUB_TYPE BINARY` → `LargeBinary`
//! - `DATE`/`TIME`/`TIMESTAMP` → `Date`/`Time`/`DateTime`
//! - `BOOLEAN` → `Boolean`; `INT128` → `BigInteger` (with a comment)
//!
//! Type mapping (Pydantic):
//! - `NUMERIC`/`DECIMAL`/`DECFLOAT` → `Decimal`
//! - `DATE`/`TIME`/`TIMESTAMP` → `date`/`time`/`datetime`
//! - `BLOB SUB_TYPE BINARY` → `bytes`; `BLOB SUB_TYPE TEXT` → `str`

use super::{GeneratedFile, Target};
use crate::codegen::fbtype::FbType;
use crate::codegen::generator::GenOptions;
use crate::codegen::is_nullable;
use crate::codegen::naming::{to_pascal_case, to_snake_case};
use crate::schema::{ForeignKeyInfo, TableSchema};

pub struct PythonTarget;

/// PK columns: prefer the PRIMARY KEY constraint, fall back to the first
/// unique index (same convention as the Java target).
fn pk_columns(schema: &TableSchema) -> Vec<String> {
    if let Some(pk_constraint) = schema
        .constraints
        .iter()
        .find(|c| c.r#type.eq_ignore_ascii_case("PRIMARY KEY"))
        && let Some(idx) = schema.indexes.iter().find(|i| i.name == pk_constraint.name)
    {
        return idx
            .columns
            .split(',')
            .map(|c| c.trim().to_string())
            .collect();
    }
    schema
        .indexes
        .iter()
        .find(|i| i.unique)
        .map(|i| i.columns.split(',').map(|c| c.trim().to_string()).collect())
        .unwrap_or_default()
}

/// The SQLAlchemy column type expression (e.g. `Integer`, `Numeric(10, 2)`,
/// `String(100)`).
fn sa_type(t: &FbType) -> String {
    match t {
        FbType::SmallInt => "SmallInteger".to_string(),
        FbType::Integer => "Integer".to_string(),
        FbType::BigInt => "BigInteger".to_string(),
        FbType::Int128 => "BigInteger".to_string(),
        FbType::Numeric { precision, scale } | FbType::Decimal { precision, scale } => {
            let scale_abs = scale.abs();
            if *precision > 0 && scale_abs > 0 {
                format!("Numeric({}, {})", precision, scale_abs)
            } else if *precision > 0 {
                format!("Numeric({})", precision)
            } else {
                "Numeric".to_string()
            }
        }
        FbType::DecFloat16 | FbType::DecFloat34 => "Numeric(38, 10)".to_string(),
        FbType::Float => "Float".to_string(),
        FbType::DoublePrecision => "Double".to_string(),
        FbType::Char { size } => format!("String({size})"),
        FbType::Varchar { size } => format!("String({size})"),
        FbType::Nvarchar { size } => format!("String({size})"),
        FbType::Cstring => "String".to_string(),
        FbType::Date => "Date".to_string(),
        FbType::Time => "Time".to_string(),
        FbType::Timestamp => "DateTime".to_string(),
        FbType::Boolean => "Boolean".to_string(),
        FbType::BlobText => "Text".to_string(),
        FbType::BlobBinary | FbType::Blob => "LargeBinary".to_string(),
        FbType::Unknown(_) => "Text".to_string(),
    }
}

/// The Pydantic annotation for a column (e.g. `int`, `Decimal`, `datetime`).
fn pydantic_type(t: &FbType) -> String {
    match t {
        FbType::SmallInt | FbType::Integer | FbType::BigInt | FbType::Int128 => "int".to_string(),
        FbType::Numeric { .. }
        | FbType::Decimal { .. }
        | FbType::DecFloat16
        | FbType::DecFloat34 => "Decimal".to_string(),
        FbType::Float | FbType::DoublePrecision => "float".to_string(),
        FbType::Char { .. }
        | FbType::Varchar { .. }
        | FbType::Nvarchar { .. }
        | FbType::Cstring
        | FbType::BlobText => "str".to_string(),
        FbType::BlobBinary | FbType::Blob => "bytes".to_string(),
        FbType::Date => "date".to_string(),
        FbType::Time => "time".to_string(),
        FbType::Timestamp => "datetime".to_string(),
        FbType::Boolean => "bool".to_string(),
        FbType::Unknown(_) => "str".to_string(),
    }
}

/// The SQLAlchemy import names needed from the `sqlalchemy` package
/// (deduped, sorted). Note: `DeclarativeBase`, `Mapped`, `mapped_column` and
/// `relationship` live in `sqlalchemy.orm` and are imported separately.
fn sa_imports(schema: &TableSchema) -> Vec<String> {
    let mut imports: Vec<String> = Vec::new();
    for col in &schema.columns {
        let ft = FbType::from_column(col);
        match ft {
            FbType::SmallInt => imports.push("SmallInteger".to_string()),
            FbType::Integer => imports.push("Integer".to_string()),
            FbType::BigInt | FbType::Int128 => imports.push("BigInteger".to_string()),
            FbType::Numeric { .. }
            | FbType::Decimal { .. }
            | FbType::DecFloat16
            | FbType::DecFloat34 => imports.push("Numeric".to_string()),
            FbType::Float => imports.push("Float".to_string()),
            FbType::DoublePrecision => imports.push("Double".to_string()),
            FbType::Char { .. }
            | FbType::Varchar { .. }
            | FbType::Nvarchar { .. }
            | FbType::Cstring => imports.push("String".to_string()),
            FbType::Date => imports.push("Date".to_string()),
            FbType::Time => imports.push("Time".to_string()),
            FbType::Timestamp => imports.push("DateTime".to_string()),
            FbType::Boolean => imports.push("Boolean".to_string()),
            FbType::BlobText => imports.push("Text".to_string()),
            FbType::BlobBinary | FbType::Blob => imports.push("LargeBinary".to_string()),
            FbType::Unknown(_) => imports.push("Text".to_string()),
        }
    }
    imports.sort();
    imports.dedup();
    imports
}

/// The Pydantic import names needed by the schemas (deduped, sorted).
fn pydantic_imports(schema: &TableSchema) -> Vec<String> {
    let mut imports: Vec<String> = vec![
        "BaseModel".to_string(),
        "ConfigDict".to_string(),
        "Field".to_string(),
    ];
    for col in &schema.columns {
        let ft = FbType::from_column(col);
        match ft {
            FbType::Numeric { .. }
            | FbType::Decimal { .. }
            | FbType::DecFloat16
            | FbType::DecFloat34 => imports.push("Decimal".to_string()),
            FbType::Date => imports.push("date".to_string()),
            FbType::Time => imports.push("time".to_string()),
            FbType::Timestamp => imports.push("datetime".to_string()),
            _ => {}
        }
    }
    imports.sort();
    imports.dedup();
    imports
}

impl Target for PythonTarget {
    fn type_for(&self, t: &FbType) -> String {
        sa_type(t)
    }

    fn generate(
        &self,
        schema: &TableSchema,
        fks: &[&ForeignKeyInfo],
        opts: &GenOptions,
    ) -> Vec<GeneratedFile> {
        let class_name = to_pascal_case(&schema.table);
        let module_name = to_snake_case(&schema.table);
        let pk_cols = pk_columns(schema);

        // --- models.py (SQLAlchemy) ---
        let mut b = String::new();
        if opts.include_comments {
            b.push_str(&format!(
                "\"\"\"SQLAlchemy 2.0 model for table {0}.\n\n",
                schema.table
            ));
            b.push_str("Generated by firebird-5-mcp — do not edit by hand.\"\"\"\n\n");
        }
        b.push_str("from decimal import Decimal\n\n");
        b.push_str("from datetime import date, time, datetime\n\n");
        b.push_str("from typing import Optional\n\n");
        b.push_str("from sqlalchemy import ");
        b.push_str(&sa_imports(schema).join(", "));
        b.push('\n');
        if !fks.is_empty() {
            b.push_str("from sqlalchemy import ForeignKey\n");
        }
        b.push_str(
            "from sqlalchemy.orm import DeclarativeBase, Mapped, mapped_column, relationship\n\n\n",
        );

        if !opts.is_view {
            b.push_str("class Base(DeclarativeBase):\n");
            b.push_str("    pass\n\n\n");
        }

        b.push_str(&format!("class {class_name}(Base):\n"));
        b.push_str(&format!("    __tablename__ = \"{}\"\n\n", schema.table));

        for col in &schema.columns {
            let ft = FbType::from_column(col);
            let field_name = to_snake_case(&col.name);
            let sa_t = sa_type(&ft);
            let nullable = is_nullable(col);
            let is_pk = pk_cols.contains(&col.name);

            // FK relation (include_fks): emit the relationship on the owning side
            if opts.include_fks
                && let Some(fk) = fks.iter().find(|fk| fk.column == col.name)
            {
                let ref_class = to_pascal_case(&fk.ref_table);
                b.push_str(&format!(
                    "    {field_name}: Mapped[{ref_class}] = mapped_column(ForeignKey(\"{ref_table}.{ref_column}\"))\n\n",
                    field_name = field_name,
                    ref_class = ref_class,
                    ref_table = fk.ref_table,
                    ref_column = fk.ref_column,
                ));
                continue;
            }

            // Mapped type: Optional[...] when nullable (uses the Python type,
            // not the SQLAlchemy type — SQLAlchemy 2.0 convention)
            let py_t = pydantic_type(&ft);
            let mapped = if nullable {
                format!("Mapped[Optional[{py_t}]]")
            } else {
                format!("Mapped[{py_t}]")
            };

            // Column arguments
            let mut args: Vec<String> = Vec::new();
            if is_pk {
                args.push("primary_key=True".to_string());
            }
            if !nullable && !is_pk {
                args.push("nullable=False".to_string());
            }
            // NOTE: col.default holds the raw Firebird BLR (binary) default
            // expression, which is not interpretable as Python/SQL — so we
            // deliberately do NOT emit server_default. The DB-side default
            // still applies at insert time; the model just doesn't mirror it.

            let arg_str = if args.is_empty() {
                String::new()
            } else {
                format!(", {}", args.join(", "))
            };

            b.push_str(&format!(
                "    {field_name}: {mapped} = mapped_column({sa_t}{arg_str})\n",
                field_name = field_name,
                mapped = mapped,
                sa_t = sa_t,
                arg_str = arg_str
            ));
        }

        // Pydantic schemas file
        let mut s = String::new();
        if opts.include_comments {
            s.push_str(&format!(
                "\"\"\"Pydantic v2 schemas for table {0}.\n\n",
                schema.table
            ));
            s.push_str("Generated by firebird-5-mcp — do not edit by hand.\"\"\"\n\n");
        }
        s.push_str("from decimal import Decimal\n\n");
        s.push_str("from datetime import date, time, datetime\n\n");
        s.push_str("from typing import Optional\n\n");
        s.push_str("from pydantic import ");
        s.push_str(&pydantic_imports(schema).join(", "));
        s.push_str("\n\n\n");

        // Base schema (all fields, required)
        s.push_str(&format!("class {class_name}Base(BaseModel):\n"));
        for col in &schema.columns {
            let ft = FbType::from_column(col);
            let field_name = to_snake_case(&col.name);
            let p_t = pydantic_type(&ft);
            s.push_str(&format!(
                "    {field_name}: {p_t}\n",
                field_name = field_name,
                p_t = p_t
            ));
        }
        s.push('\n');

        // Create schema (PK optional — server-generated)
        s.push_str(&format!("class {class_name}Create({class_name}Base):\n"));
        s.push_str("    pass\n\n");

        // Update schema (all fields optional)
        s.push_str(&format!("class {class_name}Update(BaseModel):\n"));
        for col in &schema.columns {
            let ft = FbType::from_column(col);
            let field_name = to_snake_case(&col.name);
            let p_t = pydantic_type(&ft);
            s.push_str(&format!(
                "    {field_name}: Optional[{p_t}] = None\n",
                field_name = field_name,
                p_t = p_t
            ));
        }
        s.push('\n');

        // Read schema (from ORM)
        s.push_str(&format!("class {class_name}Read({class_name}Base):\n"));
        s.push_str("    model_config = ConfigDict(from_attributes=True)\n");

        vec![
            GeneratedFile::new(format!("{module_name}.py"), b, "python"),
            GeneratedFile::new(format!("{module_name}_schemas.py"), s, "python-pydantic"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codegen::Language;
    use crate::schema::{ColumnInfo, ConstraintInfo, IndexInfo, TableSchema};

    fn col(name: &str, type_num: i64, sub_type: i64, size: i64, nullable: bool) -> ColumnInfo {
        ColumnInfo {
            name: name.to_string(),
            type_num,
            sub_type,
            size,
            precision: 0,
            scale: 0,
            nullable: if nullable { None } else { Some(1) },
            default: None,
            position: None,
            domain: String::new(),
        }
    }

    fn col_prec(
        name: &str,
        type_num: i64,
        sub_type: i64,
        precision: i64,
        scale: i64,
        nullable: bool,
    ) -> ColumnInfo {
        ColumnInfo {
            name: name.to_string(),
            type_num,
            sub_type,
            size: 0,
            precision,
            scale,
            nullable: if nullable { None } else { Some(1) },
            default: None,
            position: None,
            domain: String::new(),
        }
    }

    fn test_schema() -> TableSchema {
        TableSchema {
            table: "USUARIO".to_string(),
            columns: vec![
                col("ID", 8, 0, 0, false),
                col("NOME", 20, 0, 100, true),
                col("EMAIL", 20, 0, 200, true),
                col_prec("SALARIO", 16, 2, 10, 2, false),
                col("CRIADO_EM", 29, 0, 0, false),
            ],
            indexes: vec![IndexInfo {
                name: "PK_USUARIO".to_string(),
                unique: true,
                r#type: Some(0),
                columns: "ID".to_string(),
            }],
            constraints: vec![ConstraintInfo {
                name: "PK_USUARIO".to_string(),
                r#type: "PRIMARY KEY".to_string(),
            }],
            triggers: vec![],
            comments: std::collections::HashMap::new(),
        }
    }

    fn model_content(files: &[GeneratedFile]) -> &str {
        files
            .iter()
            .find(|f| f.format == "python")
            .map(|f| f.content.as_str())
            .unwrap_or("")
    }

    fn schemas_content(files: &[GeneratedFile]) -> &str {
        files
            .iter()
            .find(|f| f.format == "python-pydantic")
            .map(|f| f.content.as_str())
            .unwrap_or("")
    }

    #[test]
    fn type_for_integer_family() {
        let target = PythonTarget;
        assert_eq!(target.type_for(&FbType::SmallInt), "SmallInteger");
        assert_eq!(target.type_for(&FbType::Integer), "Integer");
        assert_eq!(target.type_for(&FbType::BigInt), "BigInteger");
        assert_eq!(target.type_for(&FbType::Int128), "BigInteger");
    }

    #[test]
    fn type_for_numeric_family() {
        let target = PythonTarget;
        assert_eq!(
            target.type_for(&FbType::Numeric {
                precision: 10,
                scale: 2
            }),
            "Numeric(10, 2)"
        );
        assert_eq!(
            target.type_for(&FbType::Decimal {
                precision: 15,
                scale: 4
            }),
            "Numeric(15, 4)"
        );
        assert_eq!(target.type_for(&FbType::DecFloat16), "Numeric(38, 10)");
        assert_eq!(target.type_for(&FbType::Float), "Float");
        assert_eq!(target.type_for(&FbType::DoublePrecision), "Double");
    }

    #[test]
    fn type_for_string_family() {
        let target = PythonTarget;
        assert_eq!(
            target.type_for(&FbType::Varchar { size: 100 }),
            "String(100)"
        );
        assert_eq!(target.type_for(&FbType::Char { size: 10 }), "String(10)");
        assert_eq!(
            target.type_for(&FbType::Nvarchar { size: 100 }),
            "String(100)"
        );
        assert_eq!(target.type_for(&FbType::BlobText), "Text");
        assert_eq!(target.type_for(&FbType::BlobBinary), "LargeBinary");
    }

    #[test]
    fn type_for_temporal_and_boolean() {
        let target = PythonTarget;
        assert_eq!(target.type_for(&FbType::Date), "Date");
        assert_eq!(target.type_for(&FbType::Time), "Time");
        assert_eq!(target.type_for(&FbType::Timestamp), "DateTime");
        assert_eq!(target.type_for(&FbType::Boolean), "Boolean");
    }

    #[test]
    fn generate_returns_two_files() {
        let target = PythonTarget;
        let schema = test_schema();
        let opts = GenOptions::new(Language::Python, false);
        let files = target.generate(&schema, &[], &opts);

        assert_eq!(files.len(), 2);
        assert_eq!(files[0].name, "usuario.py");
        assert_eq!(files[0].format, "python");
        assert_eq!(files[1].name, "usuario_schemas.py");
        assert_eq!(files[1].format, "python-pydantic");
    }

    #[test]
    fn generate_model_includes_tablename_and_columns() {
        let target = PythonTarget;
        let schema = test_schema();
        let opts = GenOptions::new(Language::Python, false);
        let files = target.generate(&schema, &[], &opts);
        let output = model_content(&files);

        assert!(output.contains("from typing import Optional"));
        assert!(output.contains("class Usuario(Base):"));
        assert!(output.contains("__tablename__ = \"USUARIO\""));
        assert!(output.contains("id: Mapped[int] = mapped_column(Integer, primary_key=True)"));
        assert!(output.contains("nome: Mapped[Optional[str]] = mapped_column(String(100))"));
        assert!(output.contains("email: Mapped[Optional[str]] = mapped_column(String(200))"));
        assert!(
            output.contains(
                "salario: Mapped[Decimal] = mapped_column(Numeric(10, 2), nullable=False)"
            )
        );
        assert!(
            output
                .contains("criado_em: Mapped[datetime] = mapped_column(DateTime, nullable=False)")
        );
    }

    #[test]
    fn generate_model_includes_base_class() {
        let target = PythonTarget;
        let schema = test_schema();
        let opts = GenOptions::new(Language::Python, false);
        let files = target.generate(&schema, &[], &opts);
        let output = model_content(&files);

        assert!(output.contains("class Base(DeclarativeBase):"));
    }

    #[test]
    fn generate_schemas_includes_base_create_update_read() {
        let target = PythonTarget;
        let schema = test_schema();
        let opts = GenOptions::new(Language::Python, false);
        let files = target.generate(&schema, &[], &opts);
        let output = schemas_content(&files);

        assert!(output.contains("class UsuarioBase(BaseModel):"));
        assert!(output.contains("class UsuarioCreate(UsuarioBase):"));
        assert!(output.contains("class UsuarioUpdate(BaseModel):"));
        assert!(output.contains("class UsuarioRead(UsuarioBase):"));
        assert!(output.contains("model_config = ConfigDict(from_attributes=True)"));
        assert!(output.contains("id: int"));
        assert!(output.contains("salario: Decimal"));
        assert!(output.contains("criado_em: datetime"));
    }

    #[test]
    fn generate_update_schema_all_optional() {
        let target = PythonTarget;
        let schema = test_schema();
        let opts = GenOptions::new(Language::Python, false);
        let files = target.generate(&schema, &[], &opts);
        let output = schemas_content(&files);

        assert!(output.contains("id: Optional[int] = None"));
        assert!(output.contains("nome: Optional[str] = None"));
        assert!(output.contains("salario: Optional[Decimal] = None"));
    }

    #[test]
    fn generate_fk_emits_relationship() {
        let target = PythonTarget;
        let schema = test_schema();
        let fk = ForeignKeyInfo {
            constraint_name: "FK_USUARIO_PESSOA".to_string(),
            table: "USUARIO".to_string(),
            column: "PESSOA_ID".to_string(),
            ref_table: "PESSOA".to_string(),
            ref_column: "ID".to_string(),
        };
        let mut schema_with_fk = schema.clone();
        schema_with_fk.columns.push(col("PESSOA_ID", 8, 0, 0, true));
        let opts = GenOptions::new(Language::Python, true);
        let files = target.generate(&schema_with_fk, &[&fk], &opts);
        let output = model_content(&files);

        assert!(
            output.contains("pessoa_id: Mapped[Pessoa] = mapped_column(ForeignKey(\"PESSOA.ID\"))")
        );
    }

    #[test]
    fn generate_does_not_leak_blr_default() {
        let target = PythonTarget;
        let mut schema = test_schema();
        // Simulate a column whose default is a raw Firebird BLR expression
        let mut col_with_default = col("TIPO", 20, 0, 400, false);
        col_with_default.default = Some(
            "blr_version5,blr_literal, blr_text2, 4,0, 6,0, 'F','I','S','I','C','O',blr_eoc"
                .to_string(),
        );
        schema.columns.push(col_with_default);
        let opts = GenOptions::new(Language::Python, false);
        let files = target.generate(&schema, &[], &opts);
        let output = model_content(&files);

        assert!(
            !output.contains("blr_version5"),
            "BLR default leaked into generated code: {output}"
        );
        assert!(!output.contains("server_default"));
        assert!(output.contains("tipo: Mapped[str] = mapped_column(String(400), nullable=False)"));
    }

    #[test]
    fn generate_view_suppresses_base_class() {
        let target = PythonTarget;
        let schema = test_schema();
        let mut opts = GenOptions::new(Language::Python, false);
        opts.is_view = true;
        let files = target.generate(&schema, &[], &opts);
        let output = model_content(&files);

        assert!(!output.contains("class Base(DeclarativeBase):"));
    }
}
