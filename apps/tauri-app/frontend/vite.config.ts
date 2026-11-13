import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Tauri 约定：固定端口、禁用清屏，并忽略 src-tauri 的变更以避免重启循环。
export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
});
