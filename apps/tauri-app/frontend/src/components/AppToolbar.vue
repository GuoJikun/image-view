<script setup lang="ts">
import { computed } from "vue";
import { useViewerStore } from "../stores/viewer";
import ToolbarButton from "./ToolbarButton.vue";

const store = useViewerStore();
const zoomText = computed(() => `${store.zoomPercent}%`);
</script>

<template>
  <header class="toolbar">
    <div class="toolbar-group">
      <ToolbarButton label="打开图片" title="打开图片 (Ctrl+O)" @click="store.openDialog()">
        <svg viewBox="0 0 20 20" aria-hidden="true">
          <path d="M3 5.5A1.5 1.5 0 0 1 4.5 4h3l1.2 1.4h6.8A1.5 1.5 0 0 1 17 6.9v7.6a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 3 14.5v-9Z" fill="none" stroke="currentColor" stroke-width="1.4" />
          <path d="M3.4 8h13.2" stroke="currentColor" stroke-width="1.4" />
        </svg>
      </ToolbarButton>
      <ToolbarButton label="上一张" title="上一张 (←)" :disabled="!store.canPrev" @click="store.step(-1)">
        <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M12.5 4.5 7 10l5.5 5.5" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" /></svg>
      </ToolbarButton>
      <ToolbarButton label="下一张" title="下一张 (→)" :disabled="!store.canNext" @click="store.step(1)">
        <svg viewBox="0 0 20 20" aria-hidden="true"><path d="m7.5 4.5 5.5 5.5-5.5 5.5" fill="none" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" /></svg>
      </ToolbarButton>
    </div>

    <span class="toolbar-divider" />

    <div class="toolbar-group">
      <ToolbarButton label="适配窗口" title="适配窗口 (0)" :active="store.view.mode === 'fit'" @click="store.setFit()">
        <svg viewBox="0 0 20 20" aria-hidden="true">
          <path d="M4 7V4h3M13 4h3v3M16 13v3h-3M7 16H4v-3" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" />
          <rect x="7" y="7.5" width="6" height="5" rx="0.8" fill="none" stroke="currentColor" stroke-width="1.2" />
        </svg>
      </ToolbarButton>
      <ToolbarButton label="实际大小" title="实际大小 (1)" :active="store.view.mode === 'actual'" @click="store.setActual()">
        <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M4 6h12M4 10h12M4 14h7" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" /></svg>
      </ToolbarButton>
      <ToolbarButton label="缩小" title="缩小 (-)" @click="store.zoomBy(0.8)">
        <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M5 10h10" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" /></svg>
      </ToolbarButton>
      <span class="zoom-readout" :title="`当前缩放 ${zoomText}`">{{ zoomText }}</span>
      <ToolbarButton label="放大" title="放大 (+)" @click="store.zoomBy(1.25)">
        <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M5 10h10M10 5v10" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" /></svg>
      </ToolbarButton>
    </div>

    <span class="toolbar-divider" />

    <div class="toolbar-group">
      <ToolbarButton label="逆时针旋转" title="逆时针旋转 (Shift+R)" @click="store.rotate(-1)">
        <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M5.5 8.5A5 5 0 1 1 5 12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" /><path d="M5.5 4.5v4h4" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" /></svg>
      </ToolbarButton>
      <ToolbarButton label="顺时针旋转" title="顺时针旋转 (R)" @click="store.rotate(1)">
        <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M14.5 8.5A5 5 0 1 0 15 12" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" /><path d="M14.5 4.5v4h-4" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" /></svg>
      </ToolbarButton>
      <ToolbarButton label="水平翻转" title="水平翻转 (H)" :active="store.view.flipX" @click="store.toggleFlipX()">
        <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M10 4v12" stroke="currentColor" stroke-width="1.2" stroke-dasharray="2 2" /><path d="M4 6v8l4-4-4-4ZM16 6v8l-4-4 4-4Z" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round" /></svg>
      </ToolbarButton>
      <ToolbarButton label="垂直翻转" title="垂直翻转 (V)" :active="store.view.flipY" @click="store.toggleFlipY()">
        <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M4 10h12" stroke="currentColor" stroke-width="1.2" stroke-dasharray="2 2" /><path d="M6 4h8l-4 4-4-4ZM6 16h8l-4-4-4 4Z" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linejoin="round" /></svg>
      </ToolbarButton>
    </div>

    <div class="toolbar-spacer" />

    <label class="sort-select" title="目录排序">
      <span>排序</span>
      <select :value="store.snapshot?.sort ?? 'name'" @change="store.changeSort(($event.target as HTMLSelectElement).value as 'name' | 'modified')">
        <option value="name">文件名</option>
        <option value="modified">修改时间</option>
      </select>
    </label>

    <ToolbarButton label="元数据面板" title="显示/隐藏元数据" :active="store.showMetadata" @click="store.showMetadata = !store.showMetadata">
      <svg viewBox="0 0 20 20" aria-hidden="true"><rect x="3.5" y="4" width="13" height="12" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.4" /><path d="M12 4v12M5.8 7h4M5.8 10h4" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" /></svg>
    </ToolbarButton>
    <ToolbarButton label="缩略图条" title="显示/隐藏缩略图" :active="store.showThumbnails" @click="store.showThumbnails = !store.showThumbnails">
      <svg viewBox="0 0 20 20" aria-hidden="true"><rect x="3" y="5" width="4" height="10" rx="1" fill="none" stroke="currentColor" stroke-width="1.3" /><rect x="8" y="5" width="4" height="10" rx="1" fill="none" stroke="currentColor" stroke-width="1.3" /><rect x="13" y="5" width="4" height="10" rx="1" fill="none" stroke="currentColor" stroke-width="1.3" /></svg>
    </ToolbarButton>
    <ToolbarButton label="在资源管理器中显示" title="在文件管理器中定位" :disabled="!store.hasImage" @click="store.revealCurrent()">
      <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M3 5.5A1.5 1.5 0 0 1 4.5 4h3l1.2 1.4h6.8A1.5 1.5 0 0 1 17 6.9v7.6a1.5 1.5 0 0 1-1.5 1.5h-11A1.5 1.5 0 0 1 3 14.5v-9Z" fill="none" stroke="currentColor" stroke-width="1.4" /></svg>
    </ToolbarButton>
  </header>
</template>
