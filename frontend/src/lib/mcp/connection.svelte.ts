/**
 * Module-level MCP connection state.
 *
 * The SSE connection lives outside of any query layer: it is a long-lived
 * transport, not a query. Uses Svelte 5 runes ($state) for reactivity.
 */

import { McpClient } from "./client";

export type ConnectionStatus = "connecting" | "connected" | "disconnected";

export interface ConnectionState {
  status: ConnectionStatus;
  error: string | null;
  client: McpClient | null;
}

let _client: McpClient | null = null;

/** Reactive connection state (module-level, shared by all components). */
export const connection: ConnectionState = $state({
  status: "connecting",
  error: null,
  client: null,
});

let started = false;

async function start(): Promise<void> {
  if (started) return;
  started = true;
  connection.status = "connecting";
  connection.error = null;
  connection.client = null;
  try {
    _client = new McpClient(MCP_BASE_URL, (status) => {
      if (status === "disconnected") {
        connection.status = "disconnected";
        connection.error = null;
      }
    });
    await _client.connect();
    connection.status = "connected";
    connection.client = _client;
  } catch (e) {
    connection.status = "disconnected";
    connection.error = e instanceof Error ? e.message : String(e);
    connection.client = null;
  }
}

/** Reconnects (e.g. after a server restart). */
export async function reconnect(): Promise<void> {
  _client?.close();
  _client = null;
  started = false;
  await start();
}

// Start the connection on module load (first import).
void start();
