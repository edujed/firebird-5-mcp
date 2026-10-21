<script lang="ts">
  import { Table } from "@edujed/jedsvelted-ui/table";
  import type { TableCol } from "@edujed/jedsvelted-ui/table";
  import { Tabs } from "@edujed/jedsvelted-ui/tabs";
  import { Button } from "@edujed/jedsvelted-ui/ui";
  import { toast } from "@edujed/jedsvelted-ui/info";
  import { t } from "../i18n";
  import { localeStore } from "@edujed/jedsvelted-ui/i18n";
  import DbSelector from "../lib/DbSelector.svelte";
  import { list, describeTable, metadataExtract } from "../lib/mcp/tools";
  import { format } from "sql-formatter";
  import { createAction } from "../lib/mcp/store.svelte";
  import type {
    ListResult,
    ListObject,
    DescribeTableResult,
    MetadataExtractResult,
  } from "../lib/mcp/tools";

  type ObjectType =
    | "TABLES"
    | "PROCEDURES"
    | "FUNCTIONS"
    | "DOMAINS"
    | "TRIGGERS"
    | "INDEXES"
    | "SEQUENCES";

  const TABS: { value: ObjectType; labelKey: "tables" | "procedures" | "functions" | "domains" | "triggers" | "indexes" | "sequences" }[] = [
    { value: "TABLES", labelKey: "tables" },
    { value: "PROCEDURES", labelKey: "procedures" },
    { value: "FUNCTIONS", labelKey: "functions" },
    { value: "DOMAINS", labelKey: "domains" },
    { value: "TRIGGERS", labelKey: "triggers" },
    { value: "INDEXES", labelKey: "indexes" },
    { value: "SEQUENCES", labelKey: "sequences" },
  ];

  let database = $state("");
  let activeTab = $state<ObjectType>("TABLES");
  let selected = $state<ListObject | null>(null);

  const listAction = createAction<ListResult>();
  const describeAction = createAction<DescribeTableResult>();
  const ddlAction = createAction<MetadataExtractResult>();

  let objects: ListObject[] = $derived(listAction.state.data?.objects ?? []);
  let detail: DescribeTableResult | null = $derived(describeAction.state.data ?? null);
  let ddl: MetadataExtractResult | null = $derived(ddlAction.state.data ?? null);

  let tabs = $derived(
    TABS.map((tb) => ({ value: tb.value, label: t(tb.labelKey, undefined, $localeStore) })),
  );

  // Object list columns (varies by tab).
  let objectCols: TableCol[] = $derived.by(() => {
    switch (activeTab) {
      case "TABLES":
        return [
          { key: "name", title: t("name", undefined, $localeStore), sortable: true },
          { key: "type", title: t("type", undefined, $localeStore), sortable: true },
        ];
      case "PROCEDURES":
        return [
          { key: "name", title: t("name", undefined, $localeStore), sortable: true },
          { key: "parameters", title: t("parameters", undefined, $localeStore) },
          { key: "returns", title: t("returns", undefined, $localeStore) },
        ];
      case "FUNCTIONS":
        return [
          { key: "name", title: t("name", undefined, $localeStore), sortable: true },
          { key: "type", title: t("type", undefined, $localeStore) },
        ];
      case "DOMAINS":
        return [
          { key: "name", title: t("name", undefined, $localeStore), sortable: true },
          { key: "type", title: t("type", undefined, $localeStore) },
          {
            key: "nullable",
            title: t("nullable", undefined, $localeStore),
            formatter: (v) => (v ? "YES" : "NO"),
          },
        ];
      case "TRIGGERS":
        return [
          { key: "name", title: t("name", undefined, $localeStore), sortable: true },
          { key: "type", title: t("type", undefined, $localeStore) },
          { key: "table", title: t("table", undefined, $localeStore) },
        ];
      case "INDEXES":
        return [
          { key: "name", title: t("name", undefined, $localeStore), sortable: true },
          { key: "table", title: t("table", undefined, $localeStore) },
          {
            key: "unique",
            title: t("unique", undefined, $localeStore),
            formatter: (v) => (v ? "YES" : "NO"),
          },
        ];
      case "SEQUENCES":
        return [{ key: "name", title: t("name", undefined, $localeStore), sortable: true }];
    }
  });

  let detailCols: TableCol[] = $derived(
    detail
      ? [
          { key: "name", title: t("name", undefined, $localeStore), sortable: true },
          { key: "type", title: t("type", undefined, $localeStore) },
          {
            key: "nullable",
            title: t("nullable", undefined, $localeStore),
            formatter: (v) => (v ? "YES" : "NO"),
          },
          {
            key: "default",
            title: t("defaultValue", undefined, $localeStore),
            formatter: (v) => (v === null || v === "" ? "-" : String(v)),
          },
          { key: "position", title: t("position", undefined, $localeStore) },
          {
            key: "domain",
            title: t("domain", undefined, $localeStore),
            formatter: (v) => (v ? String(v) : "-"),
          },
        ]
      : [],
  );

  let detailIndexes: TableCol[] = $derived(
    detail
      ? [
          { key: "name", title: t("name", undefined, $localeStore), sortable: true },
          { key: "columns", title: t("columns", undefined, $localeStore) },
          {
            key: "unique",
            title: t("unique", undefined, $localeStore),
            formatter: (v) => (v ? "YES" : "NO"),
          },
        ]
      : [],
  );

  let detailConstraints: TableCol[] = $derived(
    detail
      ? [
          { key: "name", title: t("name", undefined, $localeStore), sortable: true },
          { key: "type", title: t("type", undefined, $localeStore) },
        ]
      : [],
  );

  let detailTriggers: TableCol[] = $derived(
    detail
      ? [
          { key: "name", title: t("name", undefined, $localeStore), sortable: true },
          { key: "timing", title: t("timing", undefined, $localeStore) },
          { key: "event", title: t("event", undefined, $localeStore) },
        ]
      : [],
  );

  let formattedDdl = $derived.by(() => {
    if (!ddl) return "";
    try {
      return format(ddl.ddl, { language: "sql" });
    } catch {
      return ddl.ddl;
    }
  });

  // Load list when tab or database changes.
  $effect(() => {
    if (!database) return;
    selected = null;
    describeAction.reset();
    ddlAction.reset();
    void listAction.run(() => list(activeTab, database || undefined));
  });

  function selectObject(row: Record<string, unknown>) {
    selected = row as unknown as ListObject;
    ddlAction.reset();
    if (activeTab === "TABLES" && database) {
      void describeAction.run(() => describeTable(selected!.name, database));
    }
  }

  async function extractDdl() {
    if (!selected) return;
    const name = selected.name;
    const data = await ddlAction.run(() => metadataExtract([name], database || undefined));
    if (!data) {
      toast.error(t("serverError", undefined, $localeStore) + ": " + (ddlAction.state.error ?? ""));
    }
  }
</script>

<div class="fb-page">
  <div class="fb-toolbar">
    <DbSelector bind:value={database} />
    <div class="grow"></div>
    {#if selected}
      <Button variant="secondary" onclick={extractDdl} disabled={ddlAction.state.loading}
        >{t("extractDdl", undefined, $localeStore)}</Button
      >
    {/if}
  </div>

  {#if !database}
    <div class="fb-muted">{t("selectDatabase", undefined, $localeStore)}</div>
  {:else}
    <Tabs {tabs} bind:activeTab showContent={false} />

    <div class="fb-toolbar">
      <span class="fb-muted">
        {objects.length} {t(activeTab.toLowerCase() as "tables", undefined, $localeStore)}
      </span>
    </div>

    {#if listAction.state.loading}
      <div class="fb-muted">{t("connecting", undefined, $localeStore)}</div>
    {:else if listAction.state.error}
      <div class="fb-error">{listAction.state.error}</div>
    {:else}
      <Table
        id="object-list"
        caption={t(activeTab.toLowerCase() as "tables", undefined, $localeStore)}
        data={objects as unknown as Record<string, unknown>[]}
        columns={objectCols}
        rowKey="name"
        actions={[
          {
            title: t("view", undefined, $localeStore),
            icon: "eye",
            onClick: selectObject,
          },
        ]}
      />
    {/if}

    {#if selected}
      <hr />
      <div class="fb-toolbar">
        <strong>{selected.name}</strong>
        {#if selected.type}
          <span class="fb-muted">({selected.type})</span>
        {/if}
      </div>

      {#if activeTab === "TABLES"}
        {#if describeAction.state.loading}
          <div class="fb-muted">{t("connecting", undefined, $localeStore)}</div>
        {:else if describeAction.state.error}
          <div class="fb-error">{describeAction.state.error}</div>
        {:else if detail}
          <h4>{t("columns", undefined, $localeStore)}</h4>
          <Table
            id="detail-columns"
            caption=""
            data={detail.columns as unknown as Record<string, unknown>[]}
            columns={detailCols}
            rowKey="name"
          />

          {#if detail.indexes.length}
            <h4>{t("indexes", undefined, $localeStore)}</h4>
            <Table
              id="detail-indexes"
              caption=""
              data={detail.indexes as unknown as Record<string, unknown>[]}
              columns={detailIndexes}
              rowKey="name"
            />
          {/if}

          {#if detail.constraints.length}
            <h4>{t("constraints", undefined, $localeStore)}</h4>
            <Table
              id="detail-constraints"
              caption=""
              data={detail.constraints as unknown as Record<string, unknown>[]}
              columns={detailConstraints}
              rowKey="name"
            />
          {/if}

          {#if detail.triggers.length}
            <h4>{t("triggers", undefined, $localeStore)}</h4>
            <Table
              id="detail-triggers"
              caption=""
              data={detail.triggers as unknown as Record<string, unknown>[]}
              columns={detailTriggers}
              rowKey="name"
            />
          {/if}
        {/if}
      {/if}

      {#if ddl}
        <h4>{t("ddl", undefined, $localeStore)}</h4>
        <div class="fb-result-panel">
          <pre>{formattedDdl}</pre>
        </div>
      {/if}
    {:else}
      <div class="fb-muted">{t("noObjectSelected", undefined, $localeStore)}</div>
    {/if}
  {/if}
</div>
