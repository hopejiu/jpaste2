import { defineConfig } from "vite";
// @ts-expect-error type error without @types/node package
import process from "node:process";
const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
export default defineConfig(() => ({

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 3420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
  // 4. map react → preact/compat so react-dependent libs (wouter, etc.) work with Preact
  resolve: {
    alias: {
      "react": "preact/compat",
      "react-dom": "preact/compat",
      "react-dom/test-utils": "preact/test-utils",
    },
  },
  // 5. Fix WASM MIME type for Tauri viewer windows
  optimizeDeps: {
    exclude: ["tree-sitter", "curlconverter"],
  },
  // 6. Disable crossorigin attribute on script tags (fixes toast window in release build)
  build: {
    rollupOptions: {
      output: {
        crossOriginLoading: false,
      },
    },
  },
}));
