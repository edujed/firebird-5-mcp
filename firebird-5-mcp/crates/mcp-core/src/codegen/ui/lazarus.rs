//! Lazarus/Delphi form generation (.lfm files).
//!
//! Generates basic forms with TFBDataSet, TDataSource, and appropriate
//! DB components (TDBEdit, TDBMemo, TDBDateTimePicker, TDBCheckBox)
//! for tables and views.

use crate::codegen::naming::to_pascal_case;
use crate::codegen::view_dto::ViewField;
use crate::schema::ColumnInfo;
use crate::types::format_column_type;

/// Layout constants for form generation (in pixels).
const MARGEM_ESQUERDA: i32 = 24;
const TOP_INICIAL_CAMPOS: i32 = 100;
const ALTURA_LABEL: i32 = 15;
const ALTURA_EDIT: i32 = 23;
const ESPACO_ENTRE_ELEMENTOS: i32 = 6;
const ESPACO_ENTRE_CAMPOS: i32 = 16;
const LARGURA_PADRAO_EDIT: i32 = 250;

/// A field in a Lazarus form.
#[derive(Debug, Clone)]
pub struct LazarusField {
    pub name: String,
    pub type_str: String,
    pub size: i64,
    pub nullable: bool,
}

/// Generates a .lfm file for a table or view.
pub fn generate_lfm(object_name: &str, fields: &[LazarusField], is_view: bool) -> String {
    let form_name = to_pascal_case(object_name);
    let dataset_name = format!("FBDataSet{}", form_name);
    let datasource_name = format!("DataSource{}", form_name);
    let select_sql = if is_view {
        format!("SELECT * FROM {}", object_name)
    } else {
        format!("SELECT * FROM {}", object_name)
    };

    let mut lfm = String::new();

    // Form header
    lfm.push_str(&format!(
        "object Form{}: TForm{}\n",
        form_name,
        if is_view { "Consulta" } else { "Manutencao" }
    ));
    lfm.push_str("  Left = 0\n");
    lfm.push_str("  Top = 0\n");
    lfm.push_str("  Caption = 'Form");
    lfm.push_str(&form_name);
    lfm.push_str("'\n");
    lfm.push_str("  ClientHeight = 400\n");
    lfm.push_str("  ClientWidth = 600\n");
    lfm.push_str("  LCLVersion = '3.0.0.0'\n");

    // TFBDataSet (invisible)
    lfm.push_str(&format!("  object {}: TFBDataSet\n", dataset_name));
    lfm.push_str("    Left = 24\n");
    lfm.push_str("    Top = 24\n");
    lfm.push_str("    Connected = False\n");
    lfm.push_str("    SQL.Strings = (\n");
    lfm.push_str(&format!("      '{}'\n", select_sql));
    lfm.push_str("    )\n");
    lfm.push_str("  end\n");

    // TDataSource (invisible)
    lfm.push_str(&format!("  object {}: TDataSource\n", datasource_name));
    lfm.push_str("    Left = 112\n");
    lfm.push_str("    Top = 24\n");
    lfm.push_str(&format!("    DataSet = {}\n", dataset_name));
    lfm.push_str("  end\n");

    // Generate fields (labels + DB components)
    let mut top_atual = TOP_INICIAL_CAMPOS;
    let mut tab_index = 0;

    for field in fields {
        // Determine component type and width based on field type
        let (component_type, width) = determine_component(&field.type_str, field.size);

        // Generate label
        lfm.push_str(&format!("  object Label_{}: TLabel\n", field.name));
        lfm.push_str(&format!("    Left = {}\n", MARGEM_ESQUERDA));
        lfm.push_str(&format!("    Top = {}\n", top_atual));
        lfm.push_str(&format!("    Width = {}\n", width));
        lfm.push_str(&format!("    Height = {}\n", ALTURA_LABEL));
        lfm.push_str(&format!("    Caption = '{}:'\n", field.name));
        lfm.push_str("    ParentColor = False\n");
        lfm.push_str("  end\n");

        // Advance top for the DB component
        top_atual += ALTURA_LABEL + ESPACO_ENTRE_ELEMENTOS;

        // Generate DB component
        lfm.push_str(&format!(
            "  object {}_{}: {}\n",
            component_type, field.name, component_type
        ));
        lfm.push_str(&format!("    Left = {}\n", MARGEM_ESQUERDA));
        lfm.push_str(&format!("    Top = {}\n", top_atual));
        lfm.push_str(&format!("    Width = {}\n", width));
        lfm.push_str(&format!("    Height = {}\n", ALTURA_EDIT));
        lfm.push_str(&format!("    DataSource = {}\n", datasource_name));
        lfm.push_str(&format!("    DataField = '{}'\n", field.name));
        lfm.push_str(&format!("    TabOrder = {}\n", tab_index));

        // Special properties for specific component types
        if component_type == "TDBMemo" {
            lfm.push_str("    ScrollBars = ssVertical\n");
            lfm.push_str("    WordWrap = True\n");
        } else if component_type == "TDBDateTimePicker" {
            lfm.push_str("    Kind = dtkDate\n");
        }

        lfm.push_str("  end\n");

        // Advance top for next field
        top_atual += ALTURA_EDIT + ESPACO_ENTRE_CAMPOS;
        tab_index += 1;
    }

    lfm.push_str("end\n");
    lfm
}

/// Determines the appropriate DB component type and width for a field.
fn determine_component(type_str: &str, size: i64) -> (&'static str, i32) {
    let type_upper = type_str.to_uppercase();

    // BLOB TEXT → TDBMemo
    if type_upper.contains("BLOB") && type_upper.contains("TEXT") {
        return ("TDBMemo", 450);
    }

    // BLOB BINARY → TDBEdit (or skip, but we'll use TDBEdit for now)
    if type_upper.contains("BLOB") {
        return ("TDBEdit", 250);
    }

    // BOOLEAN → TDBCheckBox
    if type_upper.contains("BOOLEAN") {
        return ("TDBCheckBox", 150);
    }

    // DATE/TIMESTAMP → TDBDateTimePicker
    if type_upper.contains("DATE") || type_upper.contains("TIMESTAMP") {
        return ("TDBDateTimePicker", 150);
    }

    // VARCHAR/CHAR → TDBEdit (width based on size)
    if type_upper.contains("VARCHAR") || type_upper.contains("CHAR") {
        let width = if size < 10 {
            80
        } else if size > 100 {
            450
        } else {
            LARGURA_PADRAO_EDIT
        };
        return ("TDBEdit", width);
    }

    // NUMERIC/DECIMAL/INTEGER → TDBEdit
    if type_upper.contains("NUMERIC")
        || type_upper.contains("DECIMAL")
        || type_upper.contains("INTEGER")
        || type_upper.contains("BIGINT")
        || type_upper.contains("SMALLINT")
    {
        return ("TDBEdit", 120);
    }

    // Default: TDBEdit
    ("TDBEdit", LARGURA_PADRAO_EDIT)
}

/// Creates a LazarusField from a ColumnInfo (for tables).
pub fn from_column_info(col: &ColumnInfo) -> LazarusField {
    let type_str = format_column_type(
        col.type_num,
        col.sub_type,
        col.size,
        col.precision,
        col.scale,
    );
    LazarusField {
        name: col.name.clone(),
        type_str,
        size: col.size,
        nullable: col.nullable.map(|n| n == 1).unwrap_or(false),
    }
}

/// Creates a LazarusField from a ViewField (for views).
pub fn from_view_field(view_field: &ViewField) -> LazarusField {
    // Parse size from type string (e.g., "VARCHAR(100)" → 100)
    let size = parse_size_from_type(&view_field.type_str);
    LazarusField {
        name: view_field.name.clone(),
        type_str: view_field.type_str.clone(),
        size,
        nullable: view_field.nullable,
    }
}

/// Parses the size from a type string like "VARCHAR(100)".
fn parse_size_from_type(type_str: &str) -> i64 {
    if let Some(start) = type_str.find('(') {
        if let Some(end) = type_str[start..].find(')') {
            let size_str = &type_str[start + 1..start + end];
            if let Ok(size) = size_str.parse::<i64>() {
                return size;
            }
        }
    }
    0
}
