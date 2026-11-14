<script setup lang="ts">
/**
 * 24 × 24 线性图标集（1.5 px 描边），尺寸由 `size` 控制。
 * 与设计稿 mockup.html 使用同一组路径。
 */
type IconName =
  | "folder"
  | "chevron-left"
  | "chevron-right"
  | "chevron-down"
  | "sidebar"
  | "play"
  | "pause"
  | "minus"
  | "plus"
  | "photo"
  | "warning"
  | "close"
  | "minimize"
  | "maximize"
  | "restore";

withDefaults(defineProps<{ name: IconName; size?: number }>(), { size: 17 });
</script>

<template>
  <svg
    class="icon"
    :width="size"
    :height="size"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.5"
    stroke-linecap="round"
    stroke-linejoin="round"
    aria-hidden="true"
  >
    <path v-if="name === 'folder'" d="M3 7.5A2 2 0 0 1 5 5.5h3.6l2 2H19a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
    <path v-else-if="name === 'chevron-left'" d="M14.5 6.5 9 12l5.5 5.5" />
    <path v-else-if="name === 'chevron-right'" d="M9.5 6.5 15 12l-5.5 5.5" />
    <path v-else-if="name === 'chevron-down'" d="M6.5 9.5 12 15l5.5-5.5" />
    <g v-else-if="name === 'sidebar'">
      <rect x="3.5" y="5" width="17" height="14" rx="2.5" />
      <path d="M10 5v14" />
    </g>
    <path v-else-if="name === 'play'" d="M8.5 6.2 18 12l-9.5 5.8z" />
    <path v-else-if="name === 'pause'" d="M9.5 6.5v11M14.5 6.5v11" />
    <path v-else-if="name === 'minus'" d="M6.5 12h11" />
    <path v-else-if="name === 'plus'" d="M12 6.5v11M6.5 12h11" />
    <g v-else-if="name === 'photo'">
      <rect x="3.5" y="5" width="17" height="14" rx="2.5" />
      <circle cx="9" cy="10" r="1.6" />
      <path d="M4.5 17.5 9.5 13l3.5 3 2.5-2.2 4 3.7" />
    </g>
    <g v-else-if="name === 'warning'">
      <path d="M12 4.2 2.9 19.5h18.2z" />
      <path d="M12 9.5v4.2" />
      <path d="M12 17.2h.01" />
    </g>
    <path v-else-if="name === 'close'" d="M6 6l12 12M18 6L6 18" />
    <path v-else-if="name === 'minimize'" d="M5 12h14" />
    <rect v-else-if="name === 'maximize'" x="5" y="5" width="14" height="14" rx="1" />
    <g v-else-if="name === 'restore'">
      <rect x="5" y="8.5" width="10.5" height="10.5" rx="1" />
      <path d="M8.5 5h10.5v10.5" />
    </g>
  </svg>
</template>
