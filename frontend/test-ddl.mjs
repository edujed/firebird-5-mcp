const BASE = 'http://127.0.0.1:9000';
const res = await fetch(`${BASE}/sse`, { headers: { Accept: 'text/event-stream' } });
const reader = res.body.getReader();
const dec = new TextDecoder();
let buf = '';
let ep = null;
const pend = new Map();
(async () => {
  try {
    while (true) {
      const { value, done } = await reader.read();
      if (done) break;
      buf += dec.decode(value, { stream: true });
      let i;
      while ((i = buf.indexOf('\n\n')) !== -1) {
        const raw = buf.slice(0, i);
        buf = buf.slice(i + 2);
        let ev = 'message', data = '';
        for (const l of raw.split('\n')) {
          if (l.startsWith('event:')) ev = l.slice(6).trim();
          else if (l.startsWith('data:')) data += l.slice(5).trim();
        }
        if (ev === 'endpoint') ep = data;
        else if (ev === 'message' && data) {
          const m = JSON.parse(data);
          if (pend.has(m.id)) { pend.get(m.id)(m); pend.delete(m.id); }
        }
      }
    }
  } catch (e) { console.error('loop err', e); }
})();
let id = 0;
function req(method, params) {
  const rid = ++id;
  return new Promise((res, rej) => {
    pend.set(rid, res);
    fetch(`${BASE}${ep}`, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ jsonrpc: '2.0', id: rid, method, params }) }).catch(rej);
    setTimeout(() => { if (pend.has(rid)) { pend.delete(rid); rej(new Error('timeout')); } }, 15000);
  });
}
await new Promise(r => setTimeout(r, 500));
await req('initialize', { protocolVersion: '2024-11-05', capabilities: {}, clientInfo: { name: 't', version: '0' } });
const m = await req('tools/call', { name: 'firebird_metadata_extract', arguments: { objects: ['USERS'], database: 'test-mcp' } });
console.log('=== DDL for USERS ===');
console.log(m.result.structuredContent.ddl);
process.exit(0);
