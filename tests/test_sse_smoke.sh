#!/bin/sh
# Smoke test for the MCP SSE transport.
# Starts the server, tests the SSE flow with curl, and cleans up.

BIN="./target/release/firebird-5-mcp"
PORT=18923
LOG=/tmp/sse_test.log

# Start the server in the background
"$BIN" --sse --port "$PORT" > "$LOG" 2>&1 &
SERVER_PID=$!

# Wait for the server to start
sleep 2

echo "=== Server started (PID $SERVER_PID) ==="
echo ""

# Test 1: GET /sse — should return an SSE stream with the endpoint event
echo "=== Test 1: GET /sse (expect endpoint event) ==="
curl -s -N --max-time 3 "http://127.0.0.1:$PORT/sse" 2>&1 | head -5
echo ""
echo ""

# Test 2: GET /sse with auth (should fail with 401)
echo "=== Test 2: GET /sse without token (server has no auth, should work) ==="
curl -s -o /dev/null -w "HTTP %{http_code}" --max-time 3 "http://127.0.0.1:$PORT/sse" 2>&1
echo ""
echo ""

# Test 3: POST /messages with invalid session (should return 404)
echo "=== Test 3: POST /messages with invalid session (expect 404) ==="
curl -s -w "\nHTTP %{http_code}" -X POST "http://127.0.0.1:$PORT/messages?sessionId=00000000-0000-0000-0000-000000000000" \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"ping"}' 2>&1
echo ""
echo ""

# Test 4: POST /messages with malformed JSON (should return 400)
echo "=== Test 4: POST /messages with malformed JSON (expect 400) ==="
curl -s -w "\nHTTP %{http_code}" -X POST "http://127.0.0.1:$PORT/messages?sessionId=00000000-0000-0000-0000-000000000000" \
  -H "Content-Type: application/json" \
  -d 'not valid json' 2>&1
echo ""
echo ""

# Test 5: Full flow — open SSE, get session ID, POST initialize, check response
echo "=== Test 5: Full flow (SSE + POST initialize) ==="
# Open SSE in background, capture output
SSE_OUT=/tmp/sse_out.txt
curl -s -N --max-time 5 "http://127.0.0.1:$PORT/sse" > "$SSE_OUT" 2>&1 &
CURL_PID=$!
sleep 1

# Extract the session ID from the endpoint event
SESSION_ID=$(grep -o 'sessionId=[a-f0-9-]*' "$SSE_OUT" | head -1 | cut -d= -f2)
if [ -z "$SESSION_ID" ]; then
    echo "FAIL: could not extract session ID from SSE stream"
    cat "$SSE_OUT"
else
    echo "Session ID: $SESSION_ID"
    # POST initialize
    echo "POSTing initialize..."
    curl -s -o /dev/null -w "POST HTTP %{http_code}\n" -X POST "http://127.0.0.1:$PORT/messages?sessionId=$SESSION_ID" \
      -H "Content-Type: application/json" \
      -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' 2>&1
    sleep 1
    echo "SSE stream output:"
    cat "$SSE_OUT"
fi

# Cleanup
kill "$CURL_PID" 2>/dev/null
kill "$SERVER_PID" 2>/dev/null
wait 2>/dev/null

echo ""
echo "=== Server log ==="
cat "$LOG"
echo ""
echo "=== Done ==="
