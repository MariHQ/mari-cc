import { defineConfig } from "vite";
import react from "@vitejs/plugin-react-swc";
import path from "path";

// Single-entry build for the Mari console. The app is served locally by the
// `mari console` command (the Rust binary embeds this dist/), so there is no
// marketing site, no multi-page rollup, and no hosted control plane here.
//
// Dev: `npm run dev` runs Vite with HMR and proxies the data API + static
// runtime config to a locally running `mari console --port 4319`. Deep links
// under /console/* fall back to index.html (SPA).
const API_TARGET = process.env.MARI_API || "http://127.0.0.1:4319";

const spaFallback = () => ({
  name: "mari-console-spa-fallback",
  configureServer(server: import("vite").ViteDevServer) {
    server.middlewares.use((req, _res, next) => {
      const uri = (req.url ?? "").split("?")[0];
      const lastSeg = uri.slice(uri.lastIndexOf("/") + 1);
      if (!lastSeg.includes(".") && (uri === "/console" || uri.startsWith("/console/"))) {
        req.url = "/index.html";
      }
      next();
    });
  },
});

export default defineConfig({
  plugins: [react(), spaFallback()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "src/shared"),
      "@saas": path.resolve(__dirname, "src/saas"),
    },
  },
  server: {
    port: 4318,
    proxy: {
      "/api": { target: API_TARGET, changeOrigin: true },
      "/auth": { target: API_TARGET, changeOrigin: true },
      "/config.json": { target: API_TARGET, changeOrigin: true },
    },
  },
  build: {
    outDir: "dist",
    emptyOutDir: true,
  },
});
