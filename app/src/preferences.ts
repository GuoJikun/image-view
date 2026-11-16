/**
 * 界面偏好（主题、语言、色彩标记、缩略图条、启动时检查更新）。
 *
 * 唯一持久化位置是后端的 `settings.ron`：启动时 `load()` 读一次，
 * 之后任何变化都整体写回（后端 `SettingsStore` 自带 400 ms 防抖，这里不再去抖）。
 *
 * localStorage 只承担两件事，不再作为独立存储：
 * - 首帧缓存：IPC 返回前先按缓存把主题/语言铺上，避免闪一下默认配色；
 * - 旧版本迁移：1.4.x 及更早把主题/语言/启动检查存在 localStorage，`settings.ron` 里一直是默认值。
 *   迁移只在首次启动做一次（`image-view.prefs-migrated` 标记），之后后端值优先，
 *   否则手动改 `settings.ron` 会被旧缓存改回去。
 */
import { computed, ref, watch } from "vue";
import { localeSetting, setLocale } from "./i18n";
import * as ipc from "./services/ipc";
import { useUiStore } from "./stores/ui";
import { useViewerStore } from "./stores/viewer";
import type { UiPreferences } from "./types/ipc";

/** 1.4.x 的 localStorage 键；只在本文件读取用于迁移，缓存写入仍在 `stores/ui.ts` 与 `i18n.ts`。 */
const LEGACY_KEYS = {
  theme: "image-view.theme",
  language: "image-view.locale",
  autoUpdate: "image-view.update-on-launch",
};

/** 迁移标记：有它就说明已经转存过一次，此后一律以后端为准。 */
const MIGRATED_KEY = "image-view.prefs-migrated";

function storage(): Storage | null {
  try {
    return localStorage;
  } catch {
    // 隐私模式或受限 WebView 下没有可用的本地存储。
    return null;
  }
}

function readLegacy(): Partial<UiPreferences> {
  const legacy: Partial<UiPreferences> = {};
  const store = storage();
  if (!store) return legacy;

  const theme = store.getItem(LEGACY_KEYS.theme);
  if (theme === "system" || theme === "light" || theme === "dark") legacy.theme = theme;
  const language = store.getItem(LEGACY_KEYS.language);
  if (language === "system" || language === "zh" || language === "en") legacy.language = language;
  const autoUpdate = store.getItem(LEGACY_KEYS.autoUpdate);
  if (autoUpdate === "1" || autoUpdate === "0") {
    legacy.checkUpdatesOnStartup = autoUpdate === "1";
  }
  return legacy;
}

function migrationDone(): boolean {
  return storage()?.getItem(MIGRATED_KEY) !== null;
}

function markMigrated(): void {
  try {
    storage()?.setItem(MIGRATED_KEY, "1");
  } catch {
    // 写不进去就下次再迁一次，取值相同，不会出错。
  }
}

export function usePreferences() {
  const store = useViewerStore();
  const ui = useUiStore();

  const current = computed<UiPreferences>(() => ({
    showThumbnails: store.showThumbnails,
    showColorStatus: ui.showColorBadge,
    theme: ui.theme,
    language: localeSetting.value,
    checkUpdatesOnStartup: ui.checkUpdatesOnLaunch,
  }));

  /** 读取完成前不回写，也先不向原生菜单推送（否则会先用默认值重建一次菜单）。 */
  const ready = ref(false);
  /** 正在应用后端值时跳过回写，否则刚读到的值会立刻再写一遍。 */
  let applying = false;

  function apply(preferences: UiPreferences): void {
    applying = true;
    try {
      store.showThumbnails = preferences.showThumbnails;
      ui.showColorBadge = preferences.showColorStatus;
      ui.theme = preferences.theme;
      setLocale(preferences.language);
      ui.checkUpdatesOnLaunch = preferences.checkUpdatesOnStartup;
    } finally {
      applying = false;
    }
  }

  async function save(preferences: UiPreferences): Promise<void> {
    if (!ipc.isTauri()) return;
    try {
      await ipc.saveUiPreferences(preferences);
    } catch (caught) {
      // 写入失败不影响当前操作；下一次变化会再写一次完整偏好。
      console.warn("保存界面偏好失败", caught);
    }
  }

  /** 启动时调用一次：读取 `settings.ron`，首次启动时把旧版本缓存的取值带过去。 */
  async function load(): Promise<void> {
    if (ipc.isTauri()) {
      try {
        const backend = await ipc.uiPreferences();
        // 迁移：1.4.x 的界面里色彩标记是写死显示的（设置项没接上），
        // 文件里的 `show_color_status: false` 只是没被读过的默认值，按当时的实际表现取 true。
        const preferences = migrationDone()
          ? backend
          : { ...backend, showColorStatus: true, ...readLegacy() };
        apply(preferences);
        // 回写一次：迁移结果落进 settings.ron，之后两边同源。
        await save(preferences);
        markMigrated();
      } catch {
        // 读取失败时保持兜底默认值，不阻塞启动。
      }
    }
    ready.value = true;
  }

  // sync：`apply` 里连续改多个字段时，`applying` 正好还在生效，不会触发多次写回。
  watch(
    current,
    (preferences) => {
      if (!ready.value || applying) return;
      void save(preferences);
    },
    { flush: "sync" },
  );

  return { load, current, ready };
}
