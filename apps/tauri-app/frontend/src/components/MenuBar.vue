<script setup lang="ts">
/**
 * Windows / Linux 的 VS Code 式菜单栏：标题栏左侧文本项 + 自绘下拉。
 * 菜单内容与 macOS 原生菜单一一对应（同一套动作 id）。
 */
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useViewerStore } from "../stores/viewer";
import { useUiStore } from "../stores/ui";
import { useActions } from "../actions";
import { accel, isMac } from "../platform";
import { t, localeSetting } from "../i18n";

interface MenuEntry {
  kind: "item" | "sub" | "sep";
  id?: string;
  label?: string;
  shortcut?: string;
  checked?: boolean;
  mark?: "check" | "dot";
  danger?: boolean;
  disabled?: boolean;
  items?: MenuEntry[];
}

interface MenuGroup {
  id: string;
  label: string;
  items: MenuEntry[];
}

const store = useViewerStore();
const ui = useUiStore();
const { run } = useActions();

const root = ref<HTMLElement | null>(null);
const openId = ref<string | null>(null);
const openSubId = ref<string | null>(null);
/** 悬停切换打开的菜单：此时点击标题不应把它关掉（VS Code / macOS 行为）。 */
const hoverOpened = ref(false);

const sortOrder = computed(() => store.snapshot?.sort ?? "name");

const groups = computed<MenuGroup[]>(() => [
  {
    id: "file",
    label: t("menu.file"),
    items: [
      { kind: "item", id: "open", label: t("menu.open"), shortcut: accel("mod", "O") },
      { kind: "sep" },
      {
        kind: "item",
        id: "reveal",
        label: t("menu.reveal"),
        disabled: !store.hasImage,
      },
      {
        kind: "item",
        id: "copy-path",
        label: t("menu.copyPath"),
        shortcut: accel("mod", "shift", "C"),
        disabled: !store.hasImage,
      },
      { kind: "sep" },
      {
        kind: "item",
        id: "trash",
        label: t("menu.trash"),
        shortcut: isMac ? "⌫" : "Delete",
        danger: true,
        disabled: !store.hasImage,
      },
    ],
  },
  {
    id: "display",
    label: t("menu.view"),
    items: [
      {
        kind: "item",
        id: "toggle-thumbnails",
        label: t("menu.thumbnails"),
        mark: "check",
        checked: store.showThumbnails,
      },
      {
        kind: "item",
        id: "slideshow",
        label: t("menu.slideshow"),
        shortcut: "Space",
        mark: "check",
        checked: store.slideshow,
        disabled: !store.hasImage,
      },
      { kind: "sep" },
      {
        kind: "sub",
        id: "sort",
        label: t("menu.sort"),
        items: [
          {
            kind: "item",
            id: "sort-name",
            label: t("menu.sortName"),
            mark: "dot",
            checked: sortOrder.value === "name",
          },
          {
            kind: "item",
            id: "sort-modified",
            label: t("menu.sortModified"),
            mark: "dot",
            checked: sortOrder.value === "modified",
          },
        ],
      },
      { kind: "sep" },
      { kind: "item", id: "zoom-in", label: t("menu.zoomIn"), shortcut: accel("mod", "="), disabled: !store.hasImage },
      { kind: "item", id: "zoom-out", label: t("menu.zoomOut"), shortcut: accel("mod", "-"), disabled: !store.hasImage },
      { kind: "item", id: "fit", label: t("menu.fit"), shortcut: accel("mod", "0"), disabled: !store.hasImage },
      { kind: "item", id: "actual", label: t("menu.actual"), shortcut: accel("mod", "1"), disabled: !store.hasImage },
      { kind: "sep" },
      {
        kind: "sub",
        id: "theme",
        label: t("menu.theme"),
        items: [
          { kind: "item", id: "theme-system", label: t("menu.themeSystem"), mark: "dot", checked: ui.theme === "system" },
          { kind: "item", id: "theme-light", label: t("menu.themeLight"), mark: "dot", checked: ui.theme === "light" },
          { kind: "item", id: "theme-dark", label: t("menu.themeDark"), mark: "dot", checked: ui.theme === "dark" },
        ],
      },
      {
        kind: "sub",
        id: "language",
        label: t("menu.language"),
        items: [
          {
            kind: "item",
            id: "lang-system",
            label: t("menu.langSystem"),
            mark: "dot",
            checked: localeSetting.value === "system",
          },
          { kind: "item", id: "lang-zh", label: t("menu.langZh"), mark: "dot", checked: localeSetting.value === "zh" },
          { kind: "item", id: "lang-en", label: t("menu.langEn"), mark: "dot", checked: localeSetting.value === "en" },
        ],
      },
      { kind: "sep" },
      {
        kind: "item",
        id: "color-badge",
        label: t("menu.colorBadge"),
        mark: "check",
        checked: ui.showColorBadge,
      },
      {
        kind: "item",
        id: "metadata",
        label: t("menu.metadata"),
        mark: "check",
        checked: store.showMetadata,
      },
    ],
  },
  {
    id: "help",
    label: t("menu.help"),
    items: [
      { kind: "item", id: "shortcuts", label: t("menu.shortcuts"), shortcut: accel("mod", "/") },
      { kind: "item", id: "update", label: t("menu.update") },
      {
        kind: "item",
        id: "auto-update",
        label: t("menu.autoUpdate"),
        mark: "check",
        checked: ui.checkUpdatesOnLaunch,
      },
      { kind: "sep" },
      { kind: "item", id: "logs", label: t("menu.logs") },
    ],
  },
]);

function close(): void {
  openId.value = null;
  openSubId.value = null;
  hoverOpened.value = false;
}

function toggleGroup(id: string): void {
  if (openId.value === id && !hoverOpened.value) {
    close();
    return;
  }
  openId.value = id;
  openSubId.value = null;
  hoverOpened.value = false;
}

function hoverGroup(id: string): void {
  if (openId.value !== null && openId.value !== id) {
    openId.value = id;
    openSubId.value = null;
    hoverOpened.value = true;
  }
}

function toggleSub(id: string): void {
  openSubId.value = openSubId.value === id ? null : id;
}

async function choose(entry: MenuEntry): Promise<void> {
  if (!entry.id || entry.disabled) return;
  close();
  await run(entry.id);
}

function onPointerDown(event: PointerEvent): void {
  if (!root.value?.contains(event.target as Node)) close();
}

function onKeyDown(event: KeyboardEvent): void {
  if (event.key === "Escape" && openId.value !== null) {
    event.stopPropagation();
    close();
  }
}

onMounted(() => {
  document.addEventListener("pointerdown", onPointerDown);
  window.addEventListener("keydown", onKeyDown, true);
  window.addEventListener("blur", close);
});

onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onPointerDown);
  window.removeEventListener("keydown", onKeyDown, true);
  window.removeEventListener("blur", close);
});
</script>

<template>
  <nav ref="root" class="menubar" role="menubar" :aria-label="t('menu.label')">
    <div
      v-for="group in groups"
      :key="group.id"
      class="menubar-group"
      @pointerenter="hoverGroup(group.id)"
    >
      <button
        type="button"
        class="menubar-item"
        :class="{ open: openId === group.id }"
        role="menuitem"
        aria-haspopup="true"
        :aria-expanded="openId === group.id"
        @click="toggleGroup(group.id)"
      >
        {{ group.label }}
      </button>

      <div v-if="openId === group.id" class="menu-panel" role="menu">
        <template v-for="(entry, index) in group.items" :key="`${group.id}-${index}`">
          <div v-if="entry.kind === 'sep'" class="menu-separator" role="separator" />

          <div v-else-if="entry.kind === 'sub'" class="submenu-host">
            <div
              class="menu-row submenu-trigger"
              :class="{ hot: openSubId === entry.id, 'is-disabled': entry.disabled }"
              role="menuitem"
              aria-haspopup="true"
              :aria-expanded="openSubId === entry.id"
              @pointerenter="openSubId = entry.id ?? null"
              @click="toggleSub(entry.id ?? '')"
            >
              <span class="menu-gutter" />
              <span class="menu-label">{{ entry.label }}</span>
              <span class="menu-chevron">›</span>
            </div>

            <div v-if="openSubId === entry.id" class="menu-panel submenu" role="menu">
              <button
                v-for="child in entry.items ?? []"
                :key="child.id"
                type="button"
                class="menu-row"
                :class="{ 'is-disabled': child.disabled }"
                role="menuitemradio"
                :aria-checked="Boolean(child.checked)"
                :disabled="child.disabled"
                @click="choose(child)"
              >
                <span class="menu-gutter">{{ child.checked ? (child.mark === "dot" ? "•" : "✓") : "" }}</span>
                <span class="menu-label">{{ child.label }}</span>
                <span class="menu-shortcut">{{ child.shortcut }}</span>
              </button>
            </div>
          </div>

          <button
            v-else
            type="button"
            class="menu-row"
            :class="{ danger: entry.danger, 'is-disabled': entry.disabled }"
            role="menuitem"
            :disabled="entry.disabled"
            @pointerenter="openSubId = null"
            @click="choose(entry)"
          >
            <span class="menu-gutter">{{ entry.checked ? (entry.mark === "dot" ? "•" : "✓") : "" }}</span>
            <span class="menu-label">{{ entry.label }}</span>
            <span class="menu-shortcut">{{ entry.shortcut }}</span>
          </button>
        </template>
      </div>
    </div>
  </nav>
</template>
