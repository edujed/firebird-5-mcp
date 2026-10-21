// Quick SSE smoke test against the running firebird-5-mcp server.
const BASE = 'http://127.0.0.1:9000';

const res = await fetch(`${BASE}/sse`, { headers: { Accept: 'text/event-stream' } });
if (!res.ok) throw new Error(`GET /sse failed: ${res.status}`);

const reader = res.body.getReader();
const decoder = new TextDecoder();
let buf = '';
let endpoint = null;
const pending = new Map();

function pump() {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('timeout waiting for SSE event')), 15000);
    const check = () => {
      // parse complete events from buf
      let idx;
      while ((idx = buf.indexOf('\n\n')) !== -1) {
        const raw = buf.slice(0, idx);
        buf = buf.slice(idx + 2);
        let event = 'message';
        let data = '';
        for (const line of raw.split('\n')) {
          if (line.startsWith('event:')) event = line.slice(6).trim();
          else if (line.startsWith('data:')) data += line.slice(5).trim();
        }
        if (event === 'endpoint') {
          endpoint = data;
          resolve('endpoint');
          return;
        }
        if (event === 'message' && data) {
          const msg = JSON.parse(data);
          if (msg.id !== undefined && pending.has(msg.id)) {
            pending.get(msg.id)(msg);
            pending.delete(msg.id);
          }
        }
      }
    };
    (async () => {
      try {
        while (true) {
          const { value, done } = await reader.read();
          if (done) break;
          buf += decoder.decode(value, { stream: true });
          check();
        }
      } catch (e) {
        clearTimeout(timer);
        reject(e);
      }
    })();
  });
}

let id = 0;
function request(method, params) {
  const reqId = ++id;
  return new Promise((resolve, reject) => {
    pending.set(reqId, resolve);
    fetch(`${BASE}${endpoint}`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ jsonrpc: '2.0', id: reqId, method, params }),
    }).then((r) => {
      if (!r.ok) reject(new Error(`POST failed: ${r.status}`));
    }).catch(reject);
    setTimeout(() => {
      if (pending.has(reqId)) {
        pending.delete(reqId);
        reject(new Error(`timeout waiting for response to ${method}`));
      }
    }, 15000);
  });
}

// 1. initialize
await pump(); // wait for endpoint event
console.log('endpoint:', endpoint);
const init = await request('initialize', {
  protocolVersion: '2024-11-05',
  capabilities: {},
  clientInfo: { name: 'smoke-test', version: '0.0.1' },
});
console.log('initialize:', JSON.stringify(init.result.serverInfo));

// 2. tools/list
const list = await request('tools/list', {});
console.log('tools:', list.result.tools.length);

// 3. firebird_get_databases
const dbs = await request('tools/call', { name: 'firebird_get_databases', arguments: {} });
console.log('get_databases structured:', JSON.stringify(dbs.result.structuredContent, null, 2));

// 4. firebird_list TABLES
const tables = await request('tools/call', { name: 'firebird_list', arguments: { type: 'TABLES', database: 'test-mcp' } });
console.log('list TABLES structured:', JSON.stringify(tables.result.structuredContent, null, 2));

// 5. firebird_query SELECT
const q = await request('tools/call', { name: 'firebird_query', arguments: { sql: 'SELECT FIRST 3 * FROM USERS', database: 'test-mcp', max_rows: 3 } });
console.log('query isError:', q.result.isError, q.result.content?.[0]?.text?.slice(0, 200));
console.log('query structured:', JSON.stringify(q.result.structuredContent, null, 2));

// 6. firebird_describe_table
const d = await request('tools/call', { name: 'firebird_describe_table', arguments: { table_name: 'USERS', database: 'test-mcp' } });
console.log('describe_table isError:', d.result.isError, d.result.content?.[0]?.text?.slice(0, 200));
console.log('describe_table structured:', JSON.stringify(d.result.structuredContent, null, 2));

// 7. firebird_explain_plan
const e = await request('tools/call', { name: 'firebird_explain_plan', arguments: { sql: 'SELECT * FROM USERS', database: 'test-mcp' } });
console.log('explain_plan isError:', e.result.isError, e.result.content?.[0]?.text?.slice(0, 200));
console.log('explain_plan structured:', JSON.stringify(e.result.structuredContent, null, 2));

// 8. firebird_metadata_extract (single object)
const m = await request('tools/call', { name: 'firebird_metadata_extract', arguments: { objects: ['EMPLOYEE'], database: 'test-mcp' } });
console.log('metadata_extract structured keys:', Object.keys(m.result.structuredContent));
console.log('metadata_extract ddl head:', m.result.structuredContent.ddl.slice(0, 200));

console.log('\nALL OK');
process.exit(0);
