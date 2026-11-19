/**
 * 对比视图状态：2~4 个图片句柄与布局。
 *
 * 图源有两种：`fullResolution=false` 用 512px 缩略图（默认，清晰但放大有损）；
 * `true` 用全像素预览通道（后端批量解码、独立保护，不受主图 LRU 淘汰）。
 * 用户可在对比工具栏切换开关；切换时重新进入对比以准备对应图源。
 *
 * 所有槽位共享同一个 zoom/pan：用户缩放、平移任一处，全部槽位同步变化，
 * 不同尺寸图片按各自原始尺寸乘同一比例显示（各自基准 fit 缩放不同）。
 */
import { computed, reactive, ref } from "vue";
import { defineStore } from "pinia";
import type { CommandError, CompareView, GalleryEntry } from "../types/ipc";
import * as ipc from "../services/ipc";
import { previewUrl, thumbnailUrl } from "../services/ipc";
import { useViewerStore } from "./viewer";

export interface CompareSlot {
  imageId: string;
  /** 缩略图 revision（mtime）。 */
  revision: number;
  /** 全像素预览 revision；null 表示该项全像素解码失败，回退缩略图。 */
  previewRevision: number | null;
  name: string;
  width: number;
  height: number;
}

const MIN_ZOOM = 0.05;
const MAX_ZOOM = 16;

export const useCompareStore = defineStore("compare", () => {
  const viewer = useViewerStore();
  const slots = ref<CompareSlot[]>([]);
  const layout = ref<"side-by-side" | "overlay">("side-by-side");
  const error = ref<CommandError | null>(null);
  const viewport = reactive({ width: 0, height: 0 });
  /** 全像素对比开关：默认关（缩略图模式）。 */
  const fullResolution = ref(false);

  /** 共享变换：所有槽位同步。 */
  const zoom = ref(1);
  const panX = ref(0);
  const panY = ref(0);

  const count = computed(() => slots.value.length);

  /** 每个槽位在当前布局下占的视口尺寸（并排平分宽度，叠加占满）。 */
  function slotViewport(): { width: number; height: number } {
    return {
      width: layout.value === "overlay" ? viewport.width : viewport.width / Math.max(1, count.value),
      height: viewport.height,
    };
  }

  /** 按槽位尺寸算 fit 基准缩放，取所有槽位的最小值作为统一基准。 */
  function baseFitZoom(): number {
    let best = Number.POSITIVE_INFINITY;
    slots.value.forEach((slot) => {
      if (!slot.width || !slot.height) return;
      const box = slotViewport();
      const fit = Math.min(box.width / slot.width, box.height / slot.height);
      best = Math.min(best, fit);
    });
    return Number.isFinite(best) && best > 0 ? best : 1;
  }

  function resetFit(): void {
    zoom.value = 1;
    panX.value = 0;
    panY.value = 0;
  }

  function zoomBy(factor: number): void {
    zoom.value = Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, zoom.value * factor));
  }

  function panBy(dx: number, dy: number): void {
    panX.value += dx;
    panY.value += dy;
  }

  function slotUrl(slot: CompareSlot): string {
    if (slot.previewRevision !== null) {
      return previewUrl(slot.imageId, slot.previewRevision);
    }
    return thumbnailUrl(slot.imageId, 512, slot.revision);
  }

  function isFullPixel(slot: CompareSlot): boolean {
    return slot.previewRevision !== null;
  }

  function imageSize(slot: CompareSlot): { width: number; height: number } {
    const base = baseFitZoom() * zoom.value;
    return {
      width: Math.round(slot.width * base),
      height: Math.round(slot.height * base),
    };
  }

  function buildSlots(entries: GalleryEntry[], view: CompareView): void {
    layout.value = view.layout;
    slots.value = view.imageIds.map((id, index) => {
      const entry = entries.find((item) => item.id === id);
      return {
        imageId: id,
        revision: entry?.revision ?? 0,
        previewRevision: view.previews[index]?.revision ?? null,
        name: entry?.name ?? id,
        width: entry?.width ?? 0,
        height: entry?.height ?? 0,
      };
    });
    resetFit();
  }

  /** 最近一次进入对比的图库条目，供切换清晰度开关时重建。 */
  let lastEntries: GalleryEntry[] = [];

  async function enter(entries: GalleryEntry[]): Promise<boolean> {
    lastEntries = entries;
    try {
      const view = await ipc.enterCompare(
        entries.map((item) => item.id),
        fullResolution.value,
      );
      viewer.suspendView();
      buildSlots(entries, view);
      return true;
    } catch (e) {
      error.value = e as CommandError;
      return false;
    }
  }

  /** 切换清晰度：重新走后端准备图源并重建槽位。 */
  async function setFullResolution(enabled: boolean): Promise<void> {
    if (fullResolution.value === enabled) return;
    fullResolution.value = enabled;
    if (lastEntries.length > 0) await enter(lastEntries);
  }

  async function setLayout(next: "side-by-side" | "overlay"): Promise<void> {
    try {
      const view = await ipc.setCompareLayout(next);
      layout.value = view.layout;
      resetFit();
    } catch (e) {
      error.value = e as CommandError;
    }
  }

  function exit(): void {
    void ipc.exitCompare();
    slots.value = [];
    lastEntries = [];
    viewer.restoreView();
  }

  function setViewport(width: number, height: number): void {
    viewport.width = width;
    viewport.height = height;
  }

  return {
    slots,
    layout,
    error,
    viewport,
    zoom,
    panX,
    panY,
    count,
    fullResolution,
    slotUrl,
    isFullPixel,
    imageSize,
    slotViewport,
    resetFit,
    zoomBy,
    panBy,
    enter,
    setLayout,
    setFullResolution,
    exit,
    setViewport,
  };
});
