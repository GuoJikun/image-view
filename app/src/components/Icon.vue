<script setup lang="ts">
/**
 * 图标统一出口：SVG 文件放在 src/icons/，由 vite-plugin-svg-icons
 * 在构建期转成 symbol sprite 注入 HTML。这里只按名字引用并控制尺寸。
 *
 * 插件只把根 <svg> 的 viewBox 带进 <symbol>，fill/stroke/stroke-width 等属性
 * 会丢失，统一在 main.css 的 .icon 规则里补齐（1.5 px 描边 / currentColor）。
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
  | "rotate-cw"
  | "rotate-ccw"
  | "flip-h"
  | "flip-v"
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
    :style="{ width: `${size}px`, height: `${size}px` }"
    aria-hidden="true"
  >
    <use :href="`#icon-${name}`" />
  </svg>
</template>
