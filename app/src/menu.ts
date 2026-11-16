/**
 * 菜单可勾选项的当前状态。
 *
 * macOS 原生菜单（推给后端）与 Windows / Linux 自绘菜单都从这里取值，
 * 避免两套菜单各写一份「哪项该打勾」的映射后互相漂移。
 * 新增可勾选项时三处要一起改：这里、`MenuBar.vue` 的菜单结构、
 * Rust 侧 `menu.rs` 的可勾菜单项与 `MenuStateDto::checks()`。
 */
import { computed, type ComputedRef } from "vue";
import { localeSetting } from "./i18n";
import { useUiStore } from "./stores/ui";
import { useViewerStore } from "./stores/viewer";
import type { MenuState } from "./types/ipc";

export function useMenuState(): ComputedRef<MenuState> {
  const store = useViewerStore();
  const ui = useUiStore();

  return computed<MenuState>(() => ({
    thumbnails: store.showThumbnails,
    slideshow: store.slideshow,
    colorBadge: ui.showColorBadge,
    metadata: store.showMetadata,
    autoUpdate: ui.checkUpdatesOnLaunch,
    sort: store.snapshot?.sort ?? "name",
    theme: ui.theme,
    language: localeSetting.value,
  }));
}
