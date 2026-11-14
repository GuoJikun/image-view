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
    <path v-else-if="name === 'close'" d="M7.5 7.5l9 9M16.5 7.5l-9 9" />
    <path v-else-if="name === 'minimize'" d="M6 12h12" />
    <rect v-else-if="name === 'maximize'" x="7" y="7" width="10" height="10" rx="1" />
    <g v-else-if="name === 'restore'">
      <rect x="6" y="9" width="9" height="9" rx="1" />
      <path d="M9 6h9v9" />
    </g>
  </svg>
</template>
