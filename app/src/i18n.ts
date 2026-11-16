/**
 * 多语言：基于 vue-i18n（Composition 模式）的全局 composer。
 *
 * - 文案表在 `locales/zh.json` / `locales/en.json`，嵌套结构直接对应 `t("menu.open")` 的路径；
 *   两者键结构由 `MessageShape` 约束，漏翻或多写都会编译失败。
 * - 消息**在构建时预编译**（`@intlify/unplugin-vue-i18n`），运行时不需要 `new Function`，
 *   因此无需为了 i18n 放开 CSP 的 `script-src`。
 * - 语言菜单提供「跟随系统 / 简体中文 / English」，切换即时生效并写 localStorage。
 * - 插值走第二个参数：`t("update.available", { version })`，文案里写 `{version}`。
 */
import { computed, ref, watch } from "vue";
import { createI18n } from "vue-i18n";
import en from "./locales/en.json";
import zh from "./locales/zh.json";

/** 层级展平后的点号键，如 `"menu.open"`。 */
type Leaves<T> = T extends string
  ? never
  : {
      [K in keyof T & string]: T[K] extends string ? K : `${K}.${Leaves<T[K]>}`;
    }[keyof T & string];

/** 全部可用文案键；`t()` 的入参即此联合类型，拼错键名编译期就报错。 */
export type MessageKey = Leaves<typeof zh>;

/** 把 zh 的键结构放宽成 string 值，用于强制其它语言结构一致。 */
type MessageShape<T> = {
  [K in keyof T]: T[K] extends string ? string : MessageShape<T[K]>;
};

const messages: { zh: typeof zh; en: MessageShape<typeof zh> } = { zh, en };

export type LocaleSetting = "system" | "zh" | "en";
export type ResolvedLocale = "zh" | "en";

const STORAGE_KEY = "image-view.locale";

function readStored(): LocaleSetting {
  try {
    const value = localStorage.getItem(STORAGE_KEY);
    if (value === "zh" || value === "en" || value === "system") return value;
  } catch {
    // 隐私模式或受限 WebView 下退化为跟随系统。
  }
  return "system";
}

const systemLocale: ResolvedLocale = navigator.language.toLowerCase().startsWith("zh")
  ? "zh"
  : "en";

/** 语言菜单的当前选择；`system` 表示跟随系统。 */
export const localeSetting = ref<LocaleSetting>(readStored());

export const resolvedLocale = computed<ResolvedLocale>(() =>
  localeSetting.value === "system" ? systemLocale : localeSetting.value,
);

export const i18n = createI18n({
  legacy: false,
  locale: resolvedLocale.value,
  // 缺译文时退回英文，而不是把键名显示给用户。
  fallbackLocale: "en",
  messages,
});

export type TranslateParams = Record<string, string | number>;

/**
 * 取词。在渲染函数或 `computed` 里调用会随语言切换自动重算。
 *
 * - 插值：`t("status.frameCount", { index, total })`，文案里写 `{index}`；
 * - 复数：`t("status.imageCount", n)`，文案里写 `{count} image | {count} images`
 *   （中文无复数变化，两份写一样即可）。
 */
export function t(key: MessageKey, args?: TranslateParams | number): string {
  if (args === undefined) return i18n.global.t(key);
  if (typeof args === "number") return i18n.global.t(key, args);
  return i18n.global.t(key, args);
}

function applyLocale(): void {
  i18n.global.locale.value = resolvedLocale.value;
  document.documentElement.lang = resolvedLocale.value === "zh" ? "zh-CN" : "en";
}

// immediate 覆盖启动时的首次应用。
watch(resolvedLocale, applyLocale, { immediate: true });

export function setLocale(next: LocaleSetting): void {
  localeSetting.value = next;
  try {
    localStorage.setItem(STORAGE_KEY, next);
  } catch {
    // 存储不可用时仅在本次会话生效。
  }
}
