<script lang="ts">
  import type { RouteState } from "@edujed/jedsvelted-ui/router";
  import { initTheme } from "@edujed/jedsvelted-ui/theme";
  import { initI18n } from "@edujed/jedsvelted-ui/i18n";
  import { ToastContainer } from "@edujed/jedsvelted-ui/info";
  import { Layout, HashRouter } from "@edujed/jedsvelted-ui/router";
  import { t } from "./i18n";
  import { connection, reconnect } from "./lib/mcp/connection.svelte";
  import QueryPage from "./pages/QueryPage.svelte";
  import SchemaPage from "./pages/SchemaPage.svelte";

  initTheme();
  initI18n();

  // Routes
  const routes = [
    {
      pattern: "/",
      moduleName: "query",
      title: () => t("query"),
      icon: "🔍",
      showInMenu: true,
    },
    {
      pattern: "/schema",
      moduleName: "schema",
      title: () => t("schema"),
      icon: "🗂️",
      showInMenu: true,
    },
  ];

  const router = new HashRouter();
  for (const r of routes) {
    router.add(r.pattern, () => {}, {
      moduleName: r.moduleName,
      title: r.title,
      icon: r.icon,
      showInMenu: r.showInMenu,
    });
  }
  router.init();

  let routeState: RouteState = $state(router.getState());
  router.addRouterListener(() => {
    routeState = router.getState();
  });

</script>

<Layout {router}>
  {#if connection.status === "disconnected"}
    <div class="fb-page">
      <div class="fb-error">
        {t("connectionError")}: {connection.error ?? t("disconnected")}
      </div>
      <button class="btn" onclick={() => reconnect()}>{t("reconnect")}</button>
    </div>
  {:else if routeState?.moduleName === "schema"}
    <SchemaPage />
  {:else}
    <QueryPage />
  {/if}
</Layout>
<ToastContainer />
