//! Svelte 5 page generation using @edujed/jedsvelted-ui.
//!
//! Generates:
//! - Page component with PageShell + Table (search by PK/FK)
//! - Form component with form fields (no event handling logic)

use crate::codegen::naming::to_camel_case;
use crate::codegen::view_dto::ViewField;
use crate::schema::ColumnInfo;
use crate::types::format_column_type;

/// A field in a Svelte form.
#[derive(Debug, Clone)]
pub struct SvelteField {
    pub name: String,
    pub type_str: String,
    pub size: i64,
    pub nullable: bool,
    pub is_pk: bool,
    pub is_fk: bool,
}

/// Generates a Svelte page component with Table.
pub fn generate_page(
    table_name: &str,
    fields: &[SvelteField],
    pk_field: &str,
    fk_fields: &[String],
) -> String {
    let mut out = String::new();

    // Script section
    out.push_str("<script lang=\"ts\">\n");
    out.push_str("\timport { PageShell } from '@edujed/jedsvelted-ui/pages';\n");
    out.push_str("\timport { Table } from '@edujed/jedsvelted-ui/table';\n");
    out.push_str("\timport type { TableCol, TableAction } from '@edujed/jedsvelted-ui/table';\n");
    out.push_str("\timport { localeStore } from '@edujed/jedsvelted-ui/i18n';\n");
    out.push_str("\timport { formatDate } from '@edujed/jedsvelted-ui/format';\n\n");

    // State
    out.push_str("\tlet data = $state<any[]>([]);\n");
    out.push_str("\tlet loading = $state(false);\n");
    out.push_str("\tlet error = $state<string | null>(null);\n\n");

    // Search state (PK + FKs)
    out.push_str("\t// Search filters\n");
    out.push_str(&format!(
        "\tlet search{} = $state('');\n",
        capitalize(pk_field)
    ));
    for fk in fk_fields {
        out.push_str(&format!("\tlet search{} = $state('');\n", capitalize(fk)));
    }
    out.push('\n');

    // Columns definition
    out.push_str("\tconst columns: TableCol[] = [\n");
    for field in fields {
        let key = to_camel_case(&field.name);
        let title = format_title(&field.name);
        let mut col_def = format!("\t\t{{ key: '{}', title: '{}' ", key, title);

        // Add sortable for PK
        if field.is_pk {
            col_def.push_str(", sortable: true");
        }

        // Add formatter for date/timestamp
        if field.type_str.to_uppercase().contains("DATE")
            || field.type_str.to_uppercase().contains("TIMESTAMP")
        {
            col_def.push_str(&format!(
                ", formatter: (v: string) => formatDate(v, $localeStore)"
            ));
        }

        col_def.push_str(" },\n");
        out.push_str(&col_def);
    }
    out.push_str("\t];\n\n");

    // Actions
    out.push_str("\tconst actions: TableAction[] = [\n");
    out.push_str("\t\t{ title: 'View', icon: 'eye', onClick: (row) => showDetail(row) },\n");
    out.push_str("\t\t{ title: 'Edit', icon: 'edit', onClick: (row) => editRow(row) },\n");
    out.push_str("\t\t{ title: 'Delete', icon: 'trash', onClick: (row) => deleteRow(row) },\n");
    out.push_str("\t];\n\n");

    // Search function
    out.push_str("\tasync function search() {\n");
    out.push_str("\t\tloading = true;\n");
    out.push_str("\t\terror = null;\n");
    out.push_str("\t\ttry {\n");
    out.push_str("\t\t\t// TODO: Implement API call\n");
    out.push_str(&format!(
        "\t\t\t// const response = await fetch('/api/{}?{}={{}}', [search{}]);\n",
        table_name.to_lowercase(),
        pk_field,
        capitalize(pk_field)
    ));
    out.push_str("\t\t\t// data = await response.json();\n");
    out.push_str("\t\t} catch (e) {\n");
    out.push_str("\t\t\terror = String(e);\n");
    out.push_str("\t\t} finally {\n");
    out.push_str("\t\t\tloading = false;\n");
    out.push_str("\t\t}\n");
    out.push_str("\t}\n\n");

    // Clear function
    out.push_str("\tfunction clear() {\n");
    out.push_str(&format!("\t\tsearch{} = '';\n", capitalize(pk_field)));
    for fk in fk_fields {
        out.push_str(&format!("\t\tsearch{} = '';\n", capitalize(fk)));
    }
    out.push_str("\t\tdata = [];\n");
    out.push_str("\t}\n\n");

    // Detail functions (stubs)
    out.push_str("\tfunction showDetail(row: any) {\n");
    out.push_str("\t\t// TODO: Implement detail view\n");
    out.push_str("\t\tconsole.log('Show detail:', row);\n");
    out.push_str("\t}\n\n");

    out.push_str("\tfunction editRow(row: any) {\n");
    out.push_str("\t\t// TODO: Implement edit form\n");
    out.push_str("\t\tconsole.log('Edit row:', row);\n");
    out.push_str("\t}\n\n");

    out.push_str("\tfunction deleteRow(row: any) {\n");
    out.push_str("\t\t// TODO: Implement delete confirmation\n");
    out.push_str("\t\tconsole.log('Delete row:', row);\n");
    out.push_str("\t}\n");
    out.push_str("</script>\n\n");

    // Template
    let page_title = format_title(table_name);
    out.push_str("<PageShell\n");
    out.push_str(&format!("\ttitle=\"{}\"\n", page_title));
    out.push_str("\tonSearch={search}\n");
    out.push_str("\tonClear={clear}\n");
    out.push_str("\t{loading}\n");
    out.push_str("\t{error}\n");
    out.push_str("\tskeletonVariant=\"table\"\n");
    out.push_str("\tskeletonRows={5}\n");
    out.push_str(">\n");

    // Search panel
    out.push_str("\t{#snippet searchFields()}\n");
    out.push_str("\t\t<div class=\"search-fields\">\n");
    out.push_str(&format!(
        "\t\t\t<label>\n\t\t\t\t{}\n\t\t\t\t<input type=\"text\" bind:value={{search{}}} />\n\t\t\t</label>\n",
        format_title(pk_field),
        capitalize(pk_field)
    ));
    for fk in fk_fields {
        out.push_str(&format!(
            "\t\t\t<label>\n\t\t\t\t{}\n\t\t\t\t<input type=\"text\" bind:value={{search{}}} />\n\t\t\t</label>\n",
            format_title(fk),
            capitalize(fk)
        ));
    }
    out.push_str("\t\t</div>\n");
    out.push_str("\t{/snippet}\n\n");

    // Content snippet
    out.push_str("\t{#snippet content(state)}\n");
    out.push_str("\t\t<Table\n");
    out.push_str(&format!("\t\t\tid=\"{}\"\n", table_name.to_lowercase()));
    out.push_str(&format!("\t\t\tcaption=\"{}\"\n", page_title));
    out.push_str("\t\t\t{data}\n");
    out.push_str("\t\t\t{columns}\n");
    out.push_str("\t\t\t{actions}\n");
    out.push_str(&format!("\t\t\trowKey=\"{}\"\n", to_camel_case(pk_field)));
    out.push_str(&format!(
        "\t\t\tcsvFileName=\"{}.csv\"\n",
        table_name.to_lowercase()
    ));
    out.push_str("\t\t\tonAdd={() => {}}\n");
    out.push_str("\t\t/>\n");
    out.push_str("\t{/snippet}\n\n");

    // Detail snippet (stub)
    out.push_str("\t{#snippet detailContent(state)}\n");
    out.push_str("\t\t<!-- TODO: Implement detail view -->\n");
    out.push_str("\t\t<p>Detail view for </p>\n");
    out.push_str("\t{/snippet}\n");

    out.push_str("</PageShell>\n");

    // Styles
    out.push_str("\n<style>\n");
    out.push_str("\t.search-fields {\n");
    out.push_str("\t\tdisplay: flex;\n");
    out.push_str("\t\tgap: 1rem;\n");
    out.push_str("\t\tmargin-bottom: 1rem;\n");
    out.push_str("\t}\n\n");
    out.push_str("\t.search-fields label {\n");
    out.push_str("\t\tdisplay: flex;\n");
    out.push_str("\t\tflex-direction: column;\n");
    out.push_str("\t\tgap: 0.25rem;\n");
    out.push_str("\t}\n");
    out.push_str("</style>\n");

    out
}

/// Generates a Svelte form component.
pub fn generate_form(_table_name: &str, fields: &[SvelteField]) -> String {
    let mut out = String::new();

    // Script section
    out.push_str("<script lang=\"ts\">\n");
    out.push_str("\timport {\n");
    out.push_str("\t\tEditField,\n");
    out.push_str("\t\tNumericField,\n");
    out.push_str("\t\tDateField,\n");
    out.push_str("\t\tSwitchField,\n");
    out.push_str("\t\tFormActions\n");
    out.push_str("\t} from '@edujed/jedsvelted-ui/forms';\n\n");

    // State for each field
    out.push_str("\t// Form state\n");
    for field in fields {
        let var_name = to_camel_case(&field.name);
        let default_value = default_value_for_type(&field.type_str);
        out.push_str(&format!(
            "\tlet {} = $state<{}>({});\n",
            var_name,
            rust_type_to_ts(&field.type_str),
            default_value
        ));
    }
    out.push('\n');

    // Save/Cancel stubs
    out.push_str("\tfunction save() {\n");
    out.push_str("\t\t// TODO: Implement save logic\n");
    out.push_str("\t\tconsole.log('Save form');\n");
    out.push_str("\t}\n\n");

    out.push_str("\tfunction cancel() {\n");
    out.push_str("\t\t// TODO: Implement cancel logic\n");
    out.push_str("\t\tconsole.log('Cancel form');\n");
    out.push_str("\t}\n");
    out.push_str("</script>\n\n");

    // Template
    out.push_str("<form>\n");
    for field in fields {
        let var_name = to_camel_case(&field.name);
        let label = format_title(&field.name);
        let required = if field.nullable { "" } else { "required" };

        match field.type_str.to_uppercase().as_str() {
            t if t.contains("BOOLEAN") => {
                out.push_str(&format!(
                    "\t<SwitchField label=\"{}\" bind:value={{{}{}}} {required} />\n",
                    label, var_name, ""
                ));
            }
            t if t.contains("DATE") || t.contains("TIMESTAMP") => {
                out.push_str(&format!(
                    "\t<DateField label=\"{}\" bind:value={{{}{}}} {required} />\n",
                    label, var_name, ""
                ));
            }
            t if t.contains("INTEGER")
                || t.contains("BIGINT")
                || t.contains("SMALLINT")
                || t.contains("NUMERIC")
                || t.contains("DECIMAL") =>
            {
                out.push_str(&format!(
                    "\t<NumericField label=\"{}\" bind:value={{{}{}}} {required} />\n",
                    label, var_name, ""
                ));
            }
            _ => {
                // Default: EditField
                out.push_str(&format!(
                    "\t<EditField label=\"{}\" bind:value={{{}{}}} placeholder=\"{}\" {required} />\n",
                    label,
                    var_name,
                    "",
                    label
                ));
            }
        }
    }
    out.push_str("\t<FormActions onSave={save} onCancel={cancel} />\n");
    out.push_str("</form>\n");

    // Styles
    out.push_str("\n<style>\n");
    out.push_str("\tform {\n");
    out.push_str("\t\tdisplay: flex;\n");
    out.push_str("\t\tflex-direction: column;\n");
    out.push_str("\t\tgap: 1rem;\n");
    out.push_str("\t}\n");
    out.push_str("</style>\n");

    out
}

/// Capitalizes the first letter of a string.
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}

/// Formats a field name as a title (e.g., "PESSOA_ID" → "Pessoa Id").
fn format_title(name: &str) -> String {
    let camel = to_camel_case(name);
    camel
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_default()
        + &camel[1..]
}

/// Returns a default value for a field type.
fn default_value_for_type(type_str: &str) -> &'static str {
    match type_str.to_uppercase().as_str() {
        t if t.contains("BOOLEAN") => "false",
        t if t.contains("INTEGER")
            || t.contains("BIGINT")
            || t.contains("SMALLINT")
            || t.contains("NUMERIC")
            || t.contains("DECIMAL") =>
        {
            "0"
        }
        _ => "''",
    }
}

/// Maps a Firebird type to a TypeScript type.
fn rust_type_to_ts(type_str: &str) -> &'static str {
    match type_str.to_uppercase().as_str() {
        t if t.contains("BOOLEAN") => "boolean",
        t if t.contains("INTEGER")
            || t.contains("BIGINT")
            || t.contains("SMALLINT")
            || t.contains("NUMERIC")
            || t.contains("DECIMAL") =>
        {
            "number"
        }
        t if t.contains("DATE") || t.contains("TIMESTAMP") => "string",
        _ => "string",
    }
}

/// Creates a SvelteField from a ColumnInfo.
pub fn from_column_info(col: &ColumnInfo, is_pk: bool, is_fk: bool) -> SvelteField {
    let type_str = format_column_type(
        col.type_num,
        col.sub_type,
        col.size,
        col.precision,
        col.scale,
    );
    SvelteField {
        name: col.name.clone(),
        type_str,
        size: col.size,
        nullable: col.nullable.map(|n| n == 1).unwrap_or(false),
        is_pk,
        is_fk,
    }
}

/// Creates a SvelteField from a ViewField (resolved metadata).
pub fn from_view_field(vf: &ViewField) -> SvelteField {
    SvelteField {
        name: vf.name.clone(),
        type_str: vf.type_str.clone(),
        size: 0,
        nullable: vf.nullable,
        is_pk: false,
        is_fk: false,
    }
}

/// Generates a Svelte page component for a view (read-only, no Form).
pub fn generate_page_for_view(view_name: &str, fields: &[SvelteField]) -> String {
    // Views have no PK/FK — use first column as rowKey
    let pk_field = fields.first().map(|f| f.name.clone()).unwrap_or_default();
    generate_page(view_name, fields, &pk_field, &[])
}
