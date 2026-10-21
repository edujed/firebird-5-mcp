/**
 * Firebird PowerToys — app-specific messages (English).
 * Generic messages (Save, Cancel, etc.) live in the lib's i18n module.
 */
export default {
  // App
  appTitle: "Firebird PowerToys",
  connected: "Connected",
  connecting: "Connecting…",
  disconnected: "Disconnected",
  connectionError: "Connection error",
  reconnect: "Reconnect",

  // Query page
  query: "Query",
  run: "Run",
  runHint: "Execute the SQL statement (Ctrl+Enter)",
  explainPlan: "Explain Plan",
  explainPlanHint: "Show the execution plan (statement is rolled back)",
  database: "Database",
  selectDatabase: "Select a database…",
  sqlEditorPlaceholder: "SELECT * FROM RDB$RELATIONS WHERE RDB$SYSTEM_FLAG = 0",
  results: "Results",
  rows: "rows",
  truncated: "Results truncated — use pagination to fetch the next page.",
  noResults: "No results.",
  executionPlan: "Execution plan",
  dmlResult: "Statement executed",
  page: "Page",
  of: "of",
  nextPage: "Next page",
  prevPage: "Previous page",

  // Schema page
  schema: "Schema",
  tables: "Tables",
  views: "Views",
  procedures: "Procedures",
  functions: "Functions",
  domains: "Domains",
  triggers: "Triggers",
  indexes: "Indexes",
  sequences: "Sequences",
  object: "Object",
  columns: "Columns",
  constraints: "Constraints",
  ddl: "DDL",
  extractDdl: "Extract DDL",
  extractDdlHint: "Extract the DDL (CREATE statements) for the selected object",
  noObjectSelected: "Select an object to see its details.",
  name: "Name",
  type: "Type",
  nullable: "Nullable",
  defaultValue: "Default",
  position: "Position",
  domain: "Domain",
  unique: "Unique",
  timing: "Timing",
  event: "Event",
  table: "Table",
  description: "Description",
  parameters: "Params",
  returns: "Returns",

  // Errors
  loadError: "Failed to load data",
  serverError: "Server error",
} as const;
