#!/usr/bin/env bash
# =============================================================================
# create_test_db.sh — Create and populate the test-mcp Firebird database.
#
# Usage:
#   ./create_test_db.sh
#
# Environment variables (optional):
#   MCP_BINARY   — Path to the firebird-5-mcp binary
#   DB_NAME      — Database name to use (default: test-mcp)
#   DB_PATH      — Full path for the .fdb file (default: /database/test-mcp.fdb)
#
# The MCP config is resolved by the binary itself (FIREBIRD_CONFIG env,
# ./config.json, or ~/.firebird-mcp/config.json).
#
# What it does:
#   1. Creates the .fdb file (if it doesn't exist)
#   2. Runs 01_drop_schema.sql (idempotent drop)
#   3. Runs 02_create_schema.sql (DDL: domains, sequences, tables, views)
#   4. Runs 03_create_triggers.sql (triggers, functions, procedures)
#   5. Runs 04_poulate.sql (sample data)
#   6. Verifies by listing tables and row counts
# =============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

MCP_BINARY="${MCP_BINARY:-$SCRIPT_DIR/../../target/release/firebird-5-mcp}"
DB_NAME="${DB_NAME:-test-mcp}"
DB_PATH="${DB_PATH:-/database/test-mcp.fdb}"

# --- Helpers ----------------------------------------------------------------

mcp_call() {
    local tool="$1"
    local args="$2"
    # Send initialize + notification + tools/call in a single session
    printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"create_test_db","version":"1.0"}}}\n{"jsonrpc":"2.0","method":"notifications/initialized"}\n{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"%s","arguments":%s}}\n' \
        "$tool" "$args" \
    | timeout 60 "$MCP_BINARY" 2>/dev/null \
    | python3 -c "
import sys, json
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        msg = json.loads(line)
    except json.JSONDecodeError:
        continue
    if msg.get('id') == 2:
        result = msg.get('result', {})
        if result.get('isError'):
            print('ERROR: ' + result.get('content', [{}])[0].get('text', 'unknown'))
            sys.exit(1)
        print(result.get('content', [{}])[0].get('text', ''))
        break
"
}

# --- Preflight ---------------------------------------------------------------

if [[ ! -x "$MCP_BINARY" ]]; then
    echo "ERROR: MCP binary not found or not executable: $MCP_BINARY" >&2
    echo "Set MCP_BINARY to the path of the firebird-5-mcp binary." >&2
    exit 1
fi



# --- Step 1: Create database -------------------------------------------------

echo "=== Step 1: Creating database '$DB_NAME' at $DB_PATH ==="

# Check if database already exists by trying to list tables
if mcp_call "firebird_list" "{\"type\":\"TABLES\",\"database\":\"$DB_NAME\"}" 2>/dev/null | grep -q "Tables"; then
    echo "Database '$DB_NAME' already exists. Skipping creation."
else
    mcp_call "firebird_create_database" "{\"path\":\"$DB_PATH\",\"register\":true,\"name\":\"$DB_NAME\",\"description\":\"MCP test database\"}"
    echo "Database created and registered."
fi

# --- Step 2: Drop existing schema (idempotent) --------------------------------

echo ""
echo "=== Step 2: Dropping existing schema (01_drop_schema.sql) ==="

mcp_call "firebird_run_script" "{\"database\":\"$DB_NAME\",\"path\":\"$SCRIPT_DIR/01_drop_schema.sql\"}"
echo "Drop complete."

# --- Step 3: Create schema ---------------------------------------------------

echo ""
echo "=== Step 3: Creating schema (02_create_schema.sql) ==="

mcp_call "firebird_run_script" "{\"database\":\"$DB_NAME\",\"path\":\"$SCRIPT_DIR/02_create_schema.sql\"}"
echo "Schema applied."

# --- Step 4: Create triggers, functions, procedures ---------------------------

echo ""
echo "=== Step 4: Creating triggers/procs (03_create_triggers.sql) ==="

mcp_call "firebird_run_script" "{\"database\":\"$DB_NAME\",\"path\":\"$SCRIPT_DIR/03_create_triggers.sql\"}"
echo "Triggers, functions and procedures created."

# --- Step 5: Populate data ----------------------------------------------------

echo ""
echo "=== Step 5: Populating data (04_poulate.sql) ==="

mcp_call "firebird_run_script" "{\"database\":\"$DB_NAME\",\"path\":\"$SCRIPT_DIR/04_poulate.sql\"}"
echo "Seed data loaded."

# --- Step 6: Verify -----------------------------------------------------------

echo ""
echo "=== Step 6: Verification ==="

echo ""
echo "--- Tables ---"
mcp_call "firebird_list" "{\"type\":\"TABLES\",\"database\":\"$DB_NAME\"}"

echo ""
echo "--- Views ---"
mcp_call "firebird_describe_database" "{\"database\":\"$DB_NAME\"}" | grep -i "view" || echo "(see describe_database output)"

echo ""
echo "--- Procedures ---"
mcp_call "firebird_list" "{\"type\":\"PROCEDURES\",\"database\":\"$DB_NAME\"}"

echo ""
echo "--- Functions ---"
mcp_call "firebird_list" "{\"type\":\"FUNCTIONS\",\"database\":\"$DB_NAME\"}"

echo ""
echo "--- Sequences ---"
mcp_call "firebird_list" "{\"type\":\"SEQUENCES\",\"database\":\"$DB_NAME\"}"

echo ""
echo "--- Domains ---"
mcp_call "firebird_list" "{\"type\":\"DOMAINS\",\"database\":\"$DB_NAME\"}"

echo ""
echo "--- Row counts ---"
for tbl in BRANCH DEPARTMENT USERS PROFILE PERMISSION SCREEN USER_DEPARTMENT USER_PROFILE PROFILE_PERMISSION PROFILE_SCREEN APPROVAL ACTION_LOG; do
    count=$(mcp_call "firebird_count" "{\"table\":\"$tbl\",\"database\":\"$DB_NAME\"}")
    echo "  $tbl: $count"
done

echo ""
echo "=== Done ==="
echo "Database '$DB_NAME' is ready for testing."
echo "It has been registered in config.json automatically (register=true)."
