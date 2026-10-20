#!/bin/sh
# Smoke test for the universal POST_EVENT feature.
#
# Tests:
# 1. firebird_query with on_success_event → verifies events.jsonl has the line
# 2. firebird_query with on_fail_event (invalid SQL) → verifies events.jsonl has the line
# 3. firebird_query without events → verifies no new lines added
#
# Prerequisites:
# - Binary built and installed: ~/.local/bin/firebird-5-mcp
# - Config at ~/.firebird-mcp/config.json with at least one database
# - The database must exist and be accessible

BIN="${HOME}/.local/bin/firebird-5-mcp"
EVENTS_FILE="${HOME}/.firebird-mcp/events.jsonl"
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

# Ensure the events file exists (touch it if not)
touch "$EVENTS_FILE"

# Helper: run a JSON-RPC request and return the response for id=2
run_rpc() {
    printf '%s\n' "$1" > "$TMPDIR/req.json"
    "$BIN" < "$TMPDIR/req.json" 2>/dev/null | grep '"id":2' | head -1
}

echo "=== Test 1: firebird_query with on_success_event ==="
BEFORE_LINES=$(wc -l < "$EVENTS_FILE" 2>/dev/null || echo 0)

RESPONSE=$(run_rpc '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"firebird_query","arguments":{"sql":"SELECT 1 AS TEST FROM RDB$DATABASE","on_success_event":"test_success_event"}}}')
echo "$RESPONSE"

# Check that events.jsonl has a new line with the event
sleep 1
AFTER_LINES=$(wc -l < "$EVENTS_FILE" 2>/dev/null || echo 0)
NEW_LINES=$((AFTER_LINES - BEFORE_LINES))
echo "New lines added: $NEW_LINES"

if [ "$NEW_LINES" -ge 1 ]; then
    LAST_LINE=$(tail -1 "$EVENTS_FILE")
    if echo "$LAST_LINE" | grep -q "test_success_event"; then
        echo "PASS: events.jsonl contains 'test_success_event'"
    else
        echo "FAIL: events.jsonl does not contain 'test_success_event'"
        echo "Last line: $LAST_LINE"
    fi
else
    echo "FAIL: no new lines added to events.jsonl"
fi

echo ""
echo "=== Test 2: firebird_query with on_fail_event (invalid SQL) ==="
BEFORE_LINES=$(wc -l < "$EVENTS_FILE" 2>/dev/null || echo 0)

RESPONSE=$(run_rpc '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"firebird_query","arguments":{"sql":"SELECT * FROM NONEXISTENT_TABLE","on_fail_event":"test_fail_event"}}}')
echo "$RESPONSE"

# Check that events.jsonl has a new line with the event
sleep 1
AFTER_LINES=$(wc -l < "$EVENTS_FILE" 2>/dev/null || echo 0)
NEW_LINES=$((AFTER_LINES - BEFORE_LINES))
echo "New lines added: $NEW_LINES"

if [ "$NEW_LINES" -ge 1 ]; then
    LAST_LINE=$(tail -1 "$EVENTS_FILE")
    if echo "$LAST_LINE" | grep -q "test_fail_event"; then
        echo "PASS: events.jsonl contains 'test_fail_event'"
    else
        echo "FAIL: events.jsonl does not contain 'test_fail_event'"
        echo "Last line: $LAST_LINE"
    fi
else
    echo "FAIL: no new lines added to events.jsonl"
fi

echo ""
echo "=== Test 3: firebird_query without events (no new lines) ==="
BEFORE_LINES=$(wc -l < "$EVENTS_FILE" 2>/dev/null || echo 0)

RESPONSE=$(run_rpc '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"firebird_query","arguments":{"sql":"SELECT 1 AS TEST FROM RDB$DATABASE"}}}')
echo "$RESPONSE"

# Check that no new lines were added
sleep 1
AFTER_LINES=$(wc -l < "$EVENTS_FILE" 2>/dev/null || echo 0)
NEW_LINES=$((AFTER_LINES - BEFORE_LINES))
echo "New lines added: $NEW_LINES"

if [ "$NEW_LINES" -eq 0 ]; then
    echo "PASS: no new lines added (as expected)"
else
    echo "FAIL: unexpected new lines added to events.jsonl"
fi

echo ""
echo "=== Done ==="
