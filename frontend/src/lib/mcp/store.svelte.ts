/**
 * Simple reactive state for MCP tool results.
 *
 * Uses Svelte 5 runes ($state) instead of legacy stores — the UI is
 * runes-only, so there is no need for the store protocol.
 */

import { getDatabases as fetchDatabases } from "./tools";
import type { GetDatabasesResult } from "./tools";

// ---------------------------------------------------------------------------
// Databases (cached, 5 min stale)
// ---------------------------------------------------------------------------

const DB_STALE_MS = 5 * 60 * 1000;

let _dbs: GetDatabasesResult | null = null;
let _dbsAt = 0;
let _dbsPromise: Promise<GetDatabasesResult> | null = null;

/** Reactive state for the configured databases (null until loaded). */
let databasesState = $state<GetDatabasesResult | null>(null);

/** Reactive list of configured databases (null until loaded). */
export function databases(): GetDatabasesResult | null {
  return databasesState;
}

export async function loadDatabases(force = false): Promise<GetDatabasesResult> {
  const now = Date.now();
  if (!force && _dbs && now - _dbsAt < DB_STALE_MS) return _dbs;
  if (_dbsPromise) return _dbsPromise;

  _dbsPromise = (async () => {
    try {
      const data = await fetchDatabases();
      _dbs = data;
      _dbsAt = Date.now();
      databasesState = data;
      return data;
    } finally {
      _dbsPromise = null;
    }
  })();

  return _dbsPromise;
}

// ---------------------------------------------------------------------------
// Generic "action" helper — loading / error / data
// ---------------------------------------------------------------------------

export interface ActionState<T> {
  loading: boolean;
  error: string | null;
  data: T | null;
}

export function createAction<T>(): {
  state: { loading: boolean; error: string | null; data: T | null };
  run: (fn: () => Promise<T>) => Promise<T | null>;
  reset: () => void;
} {
  const state = $state<ActionState<T>>({ loading: false, error: null, data: null });

  function reset() {
    state.loading = false;
    state.error = null;
    state.data = null;
  }

  async function run(fn: () => Promise<T>): Promise<T | null> {
    state.loading = true;
    state.error = null;
    try {
      const data = await fn();
      state.loading = false;
      state.data = data;
      return data;
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      state.loading = false;
      state.error = msg;
      return null;
    }
  }

  return { state, run, reset };
}
