/**
 * Minimal MCP client over HTTP+SSE (spec 2024-11-05).
 *
 * Uses `fetch` + manual stream reading instead of `EventSource` so that
 * auth headers (e.g. `Authorization: Bearer`) can be added later without
 * rework.
 *
 * Protocol:
 *  1. `GET {base}/sse` → SSE stream. First event is `endpoint` with
 *     `data: /messages?sessionId=<uuid>`.
 *  2. `POST {base}{endpoint}` with a JSON-RPC 2.0 body → `202 Accepted`.
 *  3. The response arrives as an SSE `message` event whose JSON body has
 *     the matching `id`.
 */

export interface McpToolResult {
  content: Array<{ type: string; text: string }>;
  structuredContent?: unknown;
  isError: boolean;
}

export interface McpTool {
  name: string;
  description?: string;
  inputSchema?: Record<string, unknown>;
}

type Pending = { resolve: (v: unknown) => void; reject: (e: Error) => void };

export class McpClient {
  private baseUrl: string;
  private endpoint: string | null = null;
  private pending = new Map<number, Pending>();
  private nextId = 0;
  private reader: ReadableStreamDefaultReader<Uint8Array> | null = null;
  private stream: ReadableStream<Uint8Array> | null = null;
  private buffer = "";
  private closed = false;
  private onStatus?: (status: "connected" | "disconnected") => void;

  constructor(baseUrl: string, onStatus?: (status: "connected" | "disconnected") => void) {
    this.baseUrl = baseUrl.replace(/\/$/, "");
    this.onStatus = onStatus;
  }

  /** Opens the SSE stream and performs the MCP handshake. */
  async connect(): Promise<void> {
    const res = await fetch(`${this.baseUrl}/sse`, {
      headers: { Accept: "text/event-stream" },
    });
    if (!res.ok || !res.body) {
      throw new Error(`GET /sse failed: ${res.status} ${res.statusText}`);
    }
    this.stream = res.body;
    this.reader = res.body.getReader();
    this.closed = false;
    this.onStatus?.("connected");
    void this.readLoop();

    // Wait for the `endpoint` event.
    await this.waitForEndpoint();

    // Handshake: initialize → tools/list (fire-and-forget; tools are
    // resolved lazily via `callTool`).
    await this.request("initialize", {
      protocolVersion: "2024-11-05",
      capabilities: {},
      clientInfo: { name: "firebird-powertoys", version: "0.1.0" },
    });
  }

  /** Closes the SSE stream and rejects all pending requests. */
  close(): void {
    this.closed = true;
    void this.reader?.cancel().catch(() => {});
    this.reader = null;
    this.stream = null;
    for (const [id, p] of this.pending) {
      p.reject(new Error("Connection closed"));
      this.pending.delete(id);
    }
    this.onStatus?.("disconnected");
  }

  /** Lists the tools exposed by the server. */
  async listTools(): Promise<McpTool[]> {
    const result = (await this.request("tools/list", {})) as {
      tools: McpTool[];
    };
    return result.tools;
  }

  /** Calls a tool and returns the MCP result (text + structuredContent). */
  async callTool(name: string, args: Record<string, unknown>): Promise<McpToolResult> {
    const result = (await this.request("tools/call", { name, arguments: args })) as McpToolResult;
    if (result.isError) {
      throw new Error(result.content?.[0]?.text ?? "Tool call failed");
    }
    return result;
  }

  // ------------------------------------------------------------------
  // Internals
  // ------------------------------------------------------------------

  private waitForEndpoint(): Promise<void> {
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error("Timeout waiting for SSE endpoint")), 15000);
      const check = () => {
        if (this.endpoint) {
          clearTimeout(timer);
          resolve();
        } else {
          setTimeout(check, 50);
        }
      };
      check();
    });
  }

  private readLoop(): Promise<void> {
    const decoder = new TextDecoder();
    return (async () => {
      try {
        while (!this.closed && this.reader) {
          const { value, done } = await this.reader.read();
          if (done) break;
          this.buffer += decoder.decode(value, { stream: true });
          this.processBuffer();
        }
      } catch (e) {
        if (!this.closed) {
          this.onStatus?.("disconnected");
          for (const [id, p] of this.pending) {
            p.reject(e instanceof Error ? e : new Error(String(e)));
            this.pending.delete(id);
          }
        }
      }
    })();
  }

  /** Parses complete SSE events out of the buffer and dispatches them. */
  private processBuffer(): void {
    let idx: number;
    while ((idx = this.buffer.indexOf("\n\n")) !== -1) {
      const raw = this.buffer.slice(0, idx);
      this.buffer = this.buffer.slice(idx + 2);

      let event = "message";
      let data = "";
      for (const line of raw.split("\n")) {
        if (line.startsWith("event:")) event = line.slice(6).trim();
        else if (line.startsWith("data:")) data += line.slice(5).trim();
      }
      if (!data) continue;

      if (event === "endpoint") {
        this.endpoint = data;
        continue;
      }
      if (event === "message") {
        try {
          const msg = JSON.parse(data);
          if (msg.id !== undefined && this.pending.has(msg.id)) {
            const p = this.pending.get(msg.id)!;
            this.pending.delete(msg.id);
            if (msg.error) {
              p.reject(new Error(msg.error.message ?? "JSON-RPC error"));
            } else {
              p.resolve(msg.result);
            }
          }
        } catch {
          // Ignore malformed frames (e.g. keep-alive noise).
        }
      }
    }
  }

  private request(method: string, params: Record<string, unknown>): Promise<unknown> {
    if (!this.endpoint) {
      return Promise.reject(new Error("Not connected (no endpoint)"));
    }
    const id = ++this.nextId;
    return new Promise((resolve, reject) => {
      this.pending.set(id, { resolve, reject });
      fetch(`${this.baseUrl}${this.endpoint}`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ jsonrpc: "2.0", id, method, params }),
      })
        .then((r) => {
          if (!r.ok) {
            this.pending.delete(id);
            reject(new Error(`POST /messages failed: ${r.status}`));
          }
        })
        .catch((e) => {
          this.pending.delete(id);
          reject(e);
        });
      // Safety timeout: reject if no response within 120s.
      setTimeout(() => {
        if (this.pending.has(id)) {
          this.pending.delete(id);
          reject(new Error(`Timeout waiting for response to ${method}`));
        }
      }, 120000);
    });
  }
}
