import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import vueI18n from "@intlify/unplugin-vue-i18n/vite";
import { fileURLToPath } from "node:url";

// Tauri 约定：固定端口、禁用清屏，并忽略 src-tauri 的变更以避免重启循环。
export default defineConfig({
  plugins: [
    vue(),
    // 构建期把 locale 文案编译成消息函数，运行时不需要 new Function；
    // 配合 runtimeOnly 换成不含编译器的 vue-i18n 运行时，CSP 才能保持 script-src 'self'。
    vueI18n({
      include: [fileURLToPath(new URL("./src/locales/*.json", import.meta.url))],
      runtimeOnly: true,
    }),
  ],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  envPrefix: ["VITE_", "TAURI_ENV_"],
  define: {
    // vue-i18n 的特性开关：不用 legacy API、不注入全局 $t、不打生产 devtools 探针，
    // 关掉后这三部分直接从打包结果里摇掉。
    __VUE_I18N_FULL_INSTALL__: false,
    __VUE_I18N_LEGACY_API__: false,
    __INTLIFY_PROD_DEVTOOLS__: false,
  },
});
