import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// URL base do servidor MCP (SSE). Fixa em dev; o build de produção será
// embutido no binário Rust (task futura).
const MCP_BASE_URL = "http://localhost:9000";

export default defineConfig({
  define: {
    // Disponível como global `MCP_BASE_URL` no bundle (substituição em tempo
    // de build — mantém o build estático, sem env vars em runtime).
    MCP_BASE_URL: JSON.stringify(MCP_BASE_URL),
  },
  plugins: [svelte()],
  server: {
    port: 5173,
  },
  build: {
    outDir: "dist",
  },
});
