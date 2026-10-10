import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**", "**/design/**"] },
  },
  build: {
    // WebView2 (Windows) é Chromium; WebKitGTK (Linux) é Safari ~16+.
    target: ["chrome110", "safari16"],
    cssMinify: true,
    chunkSizeWarningLimit: 600,
  },
});
