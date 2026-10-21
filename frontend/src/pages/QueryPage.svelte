<script lang="ts">
  import { Table } from "@edujed/jedsvelted-ui/table";
  import type { TableCol } from "@edujed/jedsvelted-ui/table";
  import { Button } from "@edujed/jedsvelted-ui/ui";
  import { toast } from "@edujed/jedsvelted-ui/info";
  import { t } from "../i18n";
  import { localeStore } from "@edujed/jedsvelted-ui/i18n";
  import SqlEditor from "../lib/SqlEditor.svelte";
  import DbSelector from "../lib/DbSelector.svelte";
  import { query, explainPlan } from "../lib/mcp/tools";
  import type { QueryResult } from "../lib/mcp/tools";
  import { createAction } from "../lib/mcp/store.svelte";

  const PAGE_SIZE = 100;

  let sql = $state("");
  let database = $state("");
  let page = $state(0);
  let plan = $state<string | null>(null);
  let dmlText = $state<string | null>(null);

  const runAction = createAction<{ result: QueryResult | null; text: string }>();
  const pageAction = createAction<{ result: QueryResult | null; text: string }>();
  const explainAction = createAction<{ plan: string }>();

  let result: QueryResult | null = $derived(
    runAction.state.data?.result ?? pageAction.state.data?.result ?? null,
  );
  let isLoading = $derived(runAction.state.loading || pageAction.state.loading);
  let error = $derived(runAction.state.error ?? pageAction.state.error);

  let columns: TableCol[] = $derived(
    (result?.columns ?? []).map((c) => ({
      key: c.name,
      title: c.name,
      sortable: true,
      formatter: (v: unknown) => (v === null ? "NULL" : String(v)),
    })),
  );

  // Table expects an array of objects; rows are arrays → map to {col: value}.
  let rows: Record<string, unknown>[] = $derived(
    (result?.rows ?? []).map((row) => {
      const obj: Record<string, unknown> = {};
      result!.columns.forEach((c, i) => (obj[c.name] = row[i]));
      return obj;
    }),
  );

  let canNext = $derived(result?.truncated ?? false);
  let canPrev = $derived(page > 0);

  async function run() {
    if (!sql.trim()) return;
    page = 0;
    plan = null;
    const data = await runAction.run(() =>
      query(sql, { database: database || undefined, maxRows: PAGE_SIZE, skip: 0 }),
    );
    if (data) {
      dmlText = data.result ? null : data.text;
    } else {
      toast.error(t("serverError", undefined, $localeStore) + ": " + (runAction.state.error ?? ""));
    }
  }

  async function showPlan() {
    if (!sql.trim()) return;
    const data = await explainAction.run(() => explainPlan(sql, database || undefined));
    if (data) {
      plan = data.plan;
      dmlText = null;
    } else {
      toast.error(t("serverError", undefined, $localeStore) + ": " + (explainAction.state.error ?? ""));
    }
  }

  async function goPage(p: number) {
    const data = await pageAction.run(() =>
      query(sql, { database: database || undefined, maxRows: PAGE_SIZE, skip: p * PAGE_SIZE }),
    );
    if (data) {
      page = p;
      dmlText = null;
    } else {
      toast.error(t("serverError", undefined, $localeStore) + ": " + (pageAction.state.error ?? ""));
    }
  }
</script>

<div class="fb-page">
  <div class="fb-toolbar">
    <DbSelector bind:value={database} />
    <div class="grow"></div>
    <Button
      variant="secondary"
      onclick={showPlan}
      disabled={!sql.trim() || explainAction.state.loading}
      >{t("explainPlan", undefined, $localeStore)}</Button
    >
    <Button variant="primary" onclick={run} disabled={!sql.trim() || runAction.state.loading}
      >{t("run", undefined, $localeStore)}</Button
    >
  </div>

  <SqlEditor bind:value={sql} onRun={run} />

  {#if error}
    <div class="fb-error">{error}</div>
  {/if}

  {#if dmlText}
    <div class="fb-result-panel">
      <strong>{t("dmlResult", undefined, $localeStore)}:</strong>
      <pre>{dmlText}</pre>
    </div>
  {/if}

  {#if plan !== null}
    <div class="fb-result-panel">
      <strong>{t("executionPlan", undefined, $localeStore)}:</strong>
      <pre>{plan}</pre>
    </div>
  {/if}

  {#if result}
    <div class="fb-toolbar">
      <span class="fb-muted">
        {t("results", undefined, $localeStore)}: {result.row_count} {t("rows", undefined, $localeStore)}
        {#if result.truncated}
          — {t("truncated", undefined, $localeStore)}
        {/if}
      </span>
      <div class="grow"></div>
      <span class="fb-muted">{t("page", undefined, $localeStore)} {page + 1}</span>
      <Button variant="secondary" size="sm" onclick={() => goPage(page - 1)} disabled={!canPrev || isLoading}
        >{t("prevPage", undefined, $localeStore)}</Button
      >
      <Button variant="secondary" size="sm" onclick={() => goPage(page + 1)} disabled={!canNext || isLoading}
        >{t("nextPage", undefined, $localeStore)}</Button
      >
    </div>

    {#if isLoading}
      <div class="fb-muted">{t("connecting", undefined, $localeStore)}</div>
    {:else}
      <Table
        id="query-results"
        caption={t("results", undefined, $localeStore)}
        data={rows}
        columns={columns}
      />
    {/if}
  {/if}
</div>
