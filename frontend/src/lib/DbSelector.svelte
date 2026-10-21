<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { SelectField } from "@edujed/jedsvelted-ui/forms";
  import { t } from "../i18n";
  import { localeStore } from "@edujed/jedsvelted-ui/i18n";
  import { databases as getDatabases, loadDatabases } from "./mcp/store.svelte";
  import { connection } from "./mcp/connection.svelte";

  let {
    value = $bindable(""),
  }: {
    value?: string;
  } = $props();

  let isLoading = $state(false);
  let error = $state<string | null>(null);

  let options = $derived.by(() => {
    const dbs = getDatabases()?.databases ?? [];
    return dbs.map((db) => ({
      key: db.name,
      label: db.is_default ? `${db.name} (default)` : db.name,
    }));
  });

  let placeholder = $derived(t("selectDatabase", undefined, $localeStore));
  let label = $derived(t("database", undefined, $localeStore));

  // Load databases when the connection becomes ready.
  $effect(() => {
    const status = connection.status;
    if (status !== "connected") return;
    untrack(async () => {
      isLoading = true;
      try {
        await loadDatabases();
      } catch (e) {
        error = e instanceof Error ? e.message : String(e);
      } finally {
        isLoading = false;
      }
    });
  });
</script>

{#if isLoading}
  <span class="fb-muted">{t("connecting", undefined, $localeStore)}</span>
{:else if error}
  <span class="fb-error">{t("connectionError", undefined, $localeStore)}: {error}</span>
{:else}
  <SelectField bind:value {options} {placeholder} {label} />
{/if}
