/**
 * Typed wrappers around the Firebird MCP tools.
 *
 * Each function calls `firebird_*` via the MCP client and returns the
 * `structuredContent` (JSON) when available, falling back to the Markdown
 * `text` otherwise.
 */

import { connection } from "./connection.svelte";
import type { McpToolResult } from "./client";

// ---------------------------------------------------------------------------
// Structured content shapes (mirror the Rust `structuredContent` payloads)
// ---------------------------------------------------------------------------

export interface DbInfo {
  name: string;
  description: string | null;
  path: string;
  is_default: boolean;
}

export interface GetDatabasesResult {
  databases: DbInfo[];
  default: string;
  build_time: string;
}

export interface QueryColumn {
  name: string;
  type: string;
  is_numeric: boolean;
}

export interface QueryResult {
  columns: QueryColumn[];
  rows: unknown[][];
  row_count: number;
  truncated: boolean;
}

export interface ExplainPlanResult {
  plan: string;
}

export interface SchemaColumn {
  name: string;
  type: string;
  nullable: boolean;
  default: string | null;
  position: number | null;
  domain: string;
}

export interface SchemaIndex {
  name: string;
  unique: boolean;
  type: number | null;
  columns: string;
}

export interface SchemaConstraint {
  name: string;
  type: string;
}

export interface SchemaTrigger {
  name: string;
  timing: string;
  event: string;
  sequence: number | null;
}

export interface DescribeTableResult {
  table: string;
  columns: SchemaColumn[];
  indexes: SchemaIndex[];
  constraints: SchemaConstraint[];
  triggers: SchemaTrigger[];
  comments: Record<string, string>;
}

export interface ListObject {
  name: string;
  type?: string;
  comment?: string;
  description?: string;
  table?: string;
  unique?: boolean;
  parameters?: number;
  returns?: number;
  default?: string;
  nullable?: boolean;
}

export interface ListResult {
  type: string;
  objects: ListObject[];
}

export interface MetadataExtractResult {
  database: string;
  objects: string[];
  ddl: string;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function client() {
  const c = connection.client;
  if (!c) throw new Error("MCP client not connected");
  return c;
}

function structured<T>(result: McpToolResult): T {
  if (result.structuredContent !== undefined) {
    return result.structuredContent as T;
  }
  throw new Error("Tool did not return structuredContent");
}

// ---------------------------------------------------------------------------
// Tool wrappers
// ---------------------------------------------------------------------------

/** `firebird_get_databases` — configured databases (for the selector). */
export async function getDatabases(): Promise<GetDatabasesResult> {
  const result = await client().callTool("firebird_get_databases", {});
  return structured<GetDatabasesResult>(result);
}

/** `firebird_query` — executes SQL. SELECT → grid data; DML → text message. */
export async function query(
  sql: string,
  opts: { database?: string; maxRows?: number; skip?: number } = {},
): Promise<{ result: QueryResult | null; text: string }> {
  const args: Record<string, unknown> = { sql };
  if (opts.database) args.database = opts.database;
  if (opts.maxRows !== undefined) args.max_rows = opts.maxRows;
  if (opts.skip !== undefined) args.skip = opts.skip;
  const result = await client().callTool("firebird_query", args);
  const text = result.content?.[0]?.text ?? "";
  if (result.structuredContent !== undefined) {
    return { result: structured<QueryResult>(result), text };
  }
  // DML/DDL path (no structured content).
  return { result: null, text };
}

/** `firebird_explain_plan` — execution plan (statement is rolled back). */
export async function explainPlan(
  sql: string,
  database?: string,
): Promise<ExplainPlanResult> {
  const args: Record<string, unknown> = { sql };
  if (database) args.database = database;
  const result = await client().callTool("firebird_explain_plan", args);
  return structured<ExplainPlanResult>(result);
}

/** `firebird_list` — lists objects of a given type. */
export async function list(
  type:
    | "TABLES"
    | "PROCEDURES"
    | "FUNCTIONS"
    | "DOMAINS"
    | "TRIGGERS"
    | "INDEXES"
    | "SEQUENCES",
  database?: string,
): Promise<ListResult> {
  const args: Record<string, unknown> = { type };
  if (database) args.database = database;
  const result = await client().callTool("firebird_list", args);
  return structured<ListResult>(result);
}

/** `firebird_describe_table` — full schema of a table/view. */
export async function describeTable(
  tableName: string,
  database?: string,
): Promise<DescribeTableResult> {
  const args: Record<string, unknown> = { table_name: tableName };
  if (database) args.database = database;
  const result = await client().callTool("firebird_describe_table", args);
  return structured<DescribeTableResult>(result);
}

/** `firebird_metadata_extract` — DDL for one or more objects. */
export async function metadataExtract(
  objects: string[],
  database?: string,
): Promise<MetadataExtractResult> {
  const args: Record<string, unknown> = { objects };
  if (database) args.database = database;
  const result = await client().callTool("firebird_metadata_extract", args);
  return structured<MetadataExtractResult>(result);
}
