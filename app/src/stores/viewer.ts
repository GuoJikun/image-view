/**
 * 查看器状态：只保存 ID、DTO、视图参数和任务状态。
 * 像素缓冲区由 Rust 持有，前端只拿 `iviewer://` 资源 URL。
 */
import { computed, reactive, ref } from "vue";
import { defineStore } from "pinia";
import type {
  AnimationInfo,
  AnimationReady,
  CommandError,
  Degraded,
  DirectorySnapshot,
  FitMode,
  PreviewReady,
  SortOrder,
} from "../types/ipc";
import * as ipc from "../services/ipc";
import { t, type MessageKey } from "../i18n";

const MIN_ZOOM = 0.01;
const MAX_ZOOM = 32;

/** 降级 code → 文案键；未知 code 回退到 decodeFailed 的描述。 */
const DEGRADED_KEYS: Record<string, MessageKey> = {
  overBudget: "error.degraded.overBudget",
  unsupported16Bit: "error.degraded.unsupported16Bit",
  decodeFailed: "error.degraded.decodeFailed",
};

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

export const useViewerStore = defineStore("viewer", () => {
  const snapshot = ref<DirectorySnapshot | null>(null);
  const preview = ref<PreviewReady | null>(null);
  const imageUrl = ref<string | null>(null);
  const loading = ref(false);
  const error = ref<CommandError | null>(null);
  const activeRequestId = ref(0);
  const initialized = ref(false);

  const viewport = reactive({ width: 0, height: 0, dpr: 1 });
  const view = reactive({
    mode: "fit" as FitMode,
    zoom: 1,
    panX: 0,
    panY: 0,
    rotation: 0,
    flipX: false,
    flipY: false,
  });

  const showMetadata = ref(false);
  /** 兜底默认值；启动后由 `preferences.ts` 从 `settings.ron` 覆盖。 */
  const showThumbnails = ref(true);
  const slideshow = ref(false);

  /** 当前图片的动画元信息；静态图为 `null`。 */
  const animation = ref<AnimationInfo | null>(null);
  /** 当前动画帧下标。 */
  const frameIndex = ref(0);
  const animationPlaying = ref(false);

  let slideshowTimer: number | undefined;
  let frameTimer: number | undefined;
  /** 已完整播放的轮数，用于按容器元数据停止有限循环。 */
  let loopsPlayed = 0;
  /** 动画事件可能先于预览就绪到达，先暂存，等预览匹配后再播放。 */
  let pendingAnimation: AnimationReady | null = null;
  /** 降级事件同样可能先于预览就绪；暂存后由 acceptPreview 应用。 */
  let pendingDegraded: Degraded | null = null;

  const fitZoom = computed(() => {
    const image = preview.value;
    if (!image || viewport.width <= 0 || viewport.height <= 0) {
      return 1;
    }
    const physicalWidth = viewport.width * viewport.dpr;
    const physicalHeight = viewport.height * viewport.dpr;
    return Math.min(
      physicalWidth / image.width,
      physicalHeight / image.height,
      1,
    );
  });

  const effectiveZoom = computed(() => {
    if (view.mode === "fit") return fitZoom.value;
    if (view.mode === "actual") return 1;
    return view.zoom;
  });

  const zoomPercent = computed(() => Math.round(effectiveZoom.value * 100));

  /** 旋转后图像的 CSS 像素尺寸（用于平移钳制）。 */
  const displayedSize = computed(() => {
    const image = preview.value;
    if (!image) return { width: 0, height: 0 };
    const scale = effectiveZoom.value / viewport.dpr;
    const swapped = view.rotation % 2 === 1;
    return {
      width: (swapped ? image.height : image.width) * scale,
      height: (swapped ? image.width : image.height) * scale,
    };
  });

  const currentIndex = computed(() => snapshot.value?.index ?? -1);
  const canPrev = computed(() => snapshot.value?.canPrev ?? false);
  const canNext = computed(() => snapshot.value?.canNext ?? false);
  const hasImage = computed(() => preview.value !== null);
  /** 播放中：动画优先，其次幻灯片（与 `Space` 的绑定一致）。 */
  const playbackActive = computed(
    () => slideshow.value || (animation.value !== null && animationPlaying.value),
  );

  function clampPan(): void {
    const limitX = Math.max(0, (displayedSize.value.width - viewport.width) / 2);
    const limitY = Math.max(0, (displayedSize.value.height - viewport.height) / 2);
    view.panX = clamp(view.panX, -limitX, limitX);
    view.panY = clamp(view.panY, -limitY, limitY);
  }

  function setViewport(width: number, height: number, dpr: number): void {
    viewport.width = width;
    viewport.height = height;
    viewport.dpr = dpr;
    clampPan();
  }

  function resetView(): void {
    view.mode = "fit";
    view.zoom = 1;
    view.panX = 0;
    view.panY = 0;
    view.rotation = 0;
    view.flipX = false;
    view.flipY = false;
  }

  /**
   * 进入对比时暂挂预览带来的视图重置：对比会请求多张预览，
   * 其中可能包含当前图片，若不暂挂会把用户的缩放/平移状态清掉。
   */
  let suspendedView: { mode: FitMode; zoom: number; panX: number; panY: number; rotation: number; flipX: boolean; flipY: boolean } | null = null;
  function suspendView(): void {
    suspendedView = { ...view };
  }
  function restoreView(): void {
    if (!suspendedView) return;
    Object.assign(view, suspendedView);
    suspendedView = null;
    clampPan();
  }

  function applySnapshot(next: DirectorySnapshot): void {
    snapshot.value = next;
  }

  function acceptPreview(payload: PreviewReady): void {
    if (snapshot.value?.current?.id !== payload.imageId) return;
    if (payload.requestId < activeRequestId.value) return;
    activeRequestId.value = payload.requestId;
    preview.value = payload;
    imageUrl.value = ipc.previewUrl(payload.imageId, payload.revision);
    loading.value = false;
    error.value = null;
    if (!suspendedView) resetView();

    // 切图后动画回到首帧；暂存事件若与当前图片匹配则在重置后立即应用。
    const queued = pendingAnimation;
    const queuedDegraded = pendingDegraded;
    resetAnimation();
    if (queued && queued.imageId === payload.imageId && queued.revision === payload.revision) {
      acceptAnimation(queued);
    }
    if (queuedDegraded && queuedDegraded.imageId === payload.imageId) {
      acceptDegraded(queuedDegraded);
    }
  }

  /** 动画帧解码完成：首帧已经在显示，这里开始逐帧播放。 */
  function acceptAnimation(payload: AnimationReady): void {
    if (payload.frameCount < 2) return;
    const image = preview.value;
    if (!image || image.imageId !== payload.imageId || image.revision !== payload.revision) {
      // 事件先于预览就绪到达：暂存到 `acceptPreview` 再应用。
      pendingAnimation = payload;
      return;
    }
    animation.value = {
      frameCount: payload.frameCount,
      loops: payload.loops,
      delays: payload.delays,
    };
    frameIndex.value = 0;
    loopsPlayed = 0;
    animationPlaying.value = true;
    scheduleFrame();
  }

  function clearFrameTimer(): void {
    if (frameTimer !== undefined) {
      window.clearTimeout(frameTimer);
      frameTimer = undefined;
    }
  }

  function resetAnimation(): void {
    clearFrameTimer();
    animation.value = null;
    frameIndex.value = 0;
    animationPlaying.value = false;
    loopsPlayed = 0;
    pendingAnimation = null;
    pendingDegraded = null;
  }

  /** 按当前帧的 delay 安排下一帧；delay 已由后端钳制到 50~1000 ms。 */
  function scheduleFrame(): void {
    clearFrameTimer();
    const info = animation.value;
    if (!info || !animationPlaying.value) return;
    const delay = info.delays[frameIndex.value] ?? 100;
    frameTimer = window.setTimeout(advanceFrame, delay);
  }

  function advanceFrame(): void {
    frameTimer = undefined;
    const info = animation.value;
    if (!info || !animationPlaying.value) return;

    const next = frameIndex.value + 1;
    if (next < info.frameCount) {
      frameIndex.value = next;
      scheduleFrame();
      return;
    }

    loopsPlayed += 1;
    if (info.loops !== null && loopsPlayed >= info.loops) {
      // 有限循环播放完停在最后一帧，与系统看图工具一致。
      animationPlaying.value = false;
      return;
    }
    frameIndex.value = 0;
    scheduleFrame();
  }

  function startAnimation(): void {
    const info = animation.value;
    if (!info || animationPlaying.value) return;
    // 有限循环播完后再按播放：从首帧重新开始。
    if (info.loops !== null && loopsPlayed >= info.loops) {
      loopsPlayed = 0;
      frameIndex.value = 0;
    }
    animationPlaying.value = true;
    scheduleFrame();
  }

  function pauseAnimation(): void {
    animationPlaying.value = false;
    clearFrameTimer();
  }

  function toggleAnimation(): void {
    if (!animation.value) return;
    if (animationPlaying.value) pauseAnimation();
    else startAnimation();
  }

  /** `Space` 与工具栏播放键：有动画时控制动画，否则控制幻灯片。 */
  function togglePlayback(): void {
    if (animation.value) toggleAnimation();
    else toggleSlideshow();
  }

  /** 第 `index` 帧的资源 URL；未打开图片时返回 null。 */
  function frameUrl(index: number): string | null {
    const image = preview.value;
    if (!image) return null;
    return ipc.frameUrl(image.imageId, image.revision, index);
  }

  function failPreview(payload: {
    requestId: number;
    imageId: string;
    code: string;
    message: string;
  }): void {
    if (snapshot.value?.current?.id !== payload.imageId) return;
    if (payload.requestId < activeRequestId.value) return;
    activeRequestId.value = payload.requestId;
    error.value = { code: payload.code, message: payload.message };
    loading.value = false;
  }

  /** 动画降级为静态首帧：复用错误横幅提示，文案按 code 本地化。 */
  function acceptDegraded(payload: Degraded): void {
    if (snapshot.value?.current?.id !== payload.imageId) return;
    if (payload.requestId < activeRequestId.value) return;
    // 预览尚未就绪时先暂存，避免被 acceptPreview 的 error 清空覆盖。
    const image = preview.value;
    if (!image || image.imageId !== payload.imageId) {
      pendingDegraded = payload;
      return;
    }
    const key = DEGRADED_KEYS[payload.code] ?? "error.degraded.decodeFailed";
    // 超预算与位深限制重试也无用；仅文件解码失败允许重试。
    const retryable = payload.code === "decodeFailed";
    error.value = { code: payload.code, message: t(key), retryable };
  }

  async function requestCurrent(): Promise<void> {
    const entry = snapshot.value?.current;
    if (!entry) {
      preview.value = null;
      imageUrl.value = null;
      loading.value = false;
      return;
    }

    loading.value = true;
    error.value = null;
    try {
      const request = await ipc.requestPreview(entry.id);
      // 小图可能在命令返回前就解码完成并发出事件；这里只保证代号不回退。
      activeRequestId.value = Math.max(activeRequestId.value, request.requestId);
    } catch (caught) {
      error.value = ipc.normalizeError(caught);
      loading.value = false;
    }
  }

  async function openPath(path: string): Promise<void> {
    stopSlideshow();
    loading.value = true;
    error.value = null;
    activeRequestId.value = 0;
    preview.value = null;
    imageUrl.value = null;
    resetAnimation();
    try {
      applySnapshot(await ipc.openPath(path));
      await requestCurrent();
    } catch (caught) {
      error.value = ipc.normalizeError(caught);
      loading.value = false;
    }
  }

  async function openDialog(): Promise<void> {
    try {
      const path = await ipc.pickImage();
      if (path) await openPath(path);
    } catch (caught) {
      error.value = ipc.normalizeError(caught);
    }
  }

  async function step(delta: number): Promise<void> {
    try {
      const next = await ipc.navigate(delta);
      if (!next) return;
      applySnapshot(next);
      await requestCurrent();
    } catch (caught) {
      error.value = ipc.normalizeError(caught);
    }
  }

  async function select(index: number): Promise<void> {
    if (index === snapshot.value?.index) return;
    try {
      const next = await ipc.selectImage(index);
      if (!next) return;
      applySnapshot(next);
      await requestCurrent();
    } catch (caught) {
      error.value = ipc.normalizeError(caught);
    }
  }

  async function changeSort(order: SortOrder): Promise<void> {
    try {
      const next = await ipc.setSort(order);
      const currentChanged = next.current?.id !== preview.value?.imageId;
      applySnapshot(next);
      if (currentChanged) {
        preview.value = null;
        imageUrl.value = null;
        await requestCurrent();
      }
    } catch (caught) {
      error.value = ipc.normalizeError(caught);
    }
  }

  function setFit(): void {
    view.mode = "fit";
    view.panX = 0;
    view.panY = 0;
  }

  function setActual(): void {
    view.mode = "actual";
    view.panX = 0;
    view.panY = 0;
    clampPan();
  }

  function toggleFitActual(): void {
    if (view.mode === "fit") setActual();
    else setFit();
  }

  /** 以画布中心为锚点的相对缩放（工具栏与键盘）。 */
  function zoomBy(factor: number): void {
    zoomAt(factor, 0, 0);
  }

  /** 以光标为锚点缩放；cursor 相对画布中心，单位为 CSS 像素。 */
  function zoomAt(factor: number, cursorX: number, cursorY: number): void {
    const image = preview.value;
    if (!image) return;
    const oldZoom = effectiveZoom.value;
    const nextZoom = clamp(oldZoom * factor, MIN_ZOOM, MAX_ZOOM);
    if (Math.abs(nextZoom - oldZoom) < 1e-6) return;

    const oldScale = oldZoom / viewport.dpr;
    const nextScale = nextZoom / viewport.dpr;
    const textureX = (cursorX - view.panX) / oldScale;
    const textureY = (cursorY - view.panY) / oldScale;

    view.mode = "custom";
    view.zoom = nextZoom;
    view.panX = cursorX - textureX * nextScale;
    view.panY = cursorY - textureY * nextScale;
    clampPan();
  }

  function panBy(deltaX: number, deltaY: number): void {
    view.panX += deltaX;
    view.panY += deltaY;
    clampPan();
  }

  function rotate(delta: number): void {
    view.rotation = (view.rotation + delta + 4) % 4;
    view.panX = 0;
    view.panY = 0;
    clampPan();
  }

  function toggleFlipX(): void {
    view.flipX = !view.flipX;
  }

  function toggleFlipY(): void {
    view.flipY = !view.flipY;
  }

  function clearError(): void {
    error.value = null;
  }

  async function revealCurrent(): Promise<void> {
    const id = snapshot.value?.current?.id;
    if (!id) return;
    try {
      await ipc.revealImage(id);
    } catch (caught) {
      error.value = ipc.normalizeError(caught);
    }
  }

  /** 把当前图片移到系统回收站；返回是否成功。 */
  async function deleteCurrent(): Promise<boolean> {
    const entry = snapshot.value?.current;
    if (!entry) return false;
    try {
      const next = await ipc.deleteImage(entry.id);
      preview.value = null;
      imageUrl.value = null;
      resetAnimation();
      applySnapshot(next);
      if (next.current) {
        await requestCurrent();
      } else {
        loading.value = false;
        resetView();
      }
      return true;
    } catch (caught) {
      error.value = ipc.normalizeError(caught);
      return false;
    }
  }

  function stopSlideshow(): void {
    slideshow.value = false;
    if (slideshowTimer !== undefined) {
      window.clearInterval(slideshowTimer);
      slideshowTimer = undefined;
    }
  }

  /** 幻灯片：每 3 s 前进一张，到末尾自动停止。 */
  function toggleSlideshow(): void {
    if (slideshow.value) {
      stopSlideshow();
      return;
    }
    if (!hasImage.value) return;
    slideshow.value = true;
    slideshowTimer = window.setInterval(() => {
      if (!canNext.value) {
        stopSlideshow();
        return;
      }
      void step(1);
    }, 3000);
  }

  async function close(): Promise<void> {
    stopSlideshow();
    try {
      await ipc.closeImage();
    } catch {
      // 关闭失败不影响界面回到空状态。
    }
    preview.value = null;
    imageUrl.value = null;
    snapshot.value = null;
    loading.value = false;
    resetAnimation();
    resetView();
  }

  async function initialize(): Promise<void> {
    if (initialized.value) return;
    initialized.value = true;

    if (!ipc.isTauri()) {
      error.value = {
        code: "not_tauri",
        message: "请通过 `npm run tauri dev` 启动，浏览器页面无法访问 Rust 后端。",
      };
      return;
    }

    await ipc.onPreviewReady(acceptPreview);
    await ipc.onPreviewFailed(failPreview);
    await ipc.onAnimationReady(acceptAnimation);
    await ipc.onDegraded(acceptDegraded);
    await ipc.onOpenFile((path) => {
      void openPath(path);
    });

    const startup = await ipc.startupPath();
    if (startup) await openPath(startup);
  }

  return {
    snapshot,
    preview,
    imageUrl,
    loading,
    error,
    initialized,
    viewport,
    view,
    showMetadata,
    showThumbnails,
    slideshow,
    animation,
    frameIndex,
    animationPlaying,
    fitZoom,
    effectiveZoom,
    zoomPercent,
    displayedSize,
    currentIndex,
    canPrev,
    canNext,
    hasImage,
    playbackActive,
    initialize,
    openPath,
    openDialog,
    step,
    select,
    changeSort,
    requestCurrent,
    setViewport,
    setFit,
    setActual,
    toggleFitActual,
    zoomBy,
    zoomAt,
    panBy,
    rotate,
    toggleFlipX,
    toggleFlipY,
    clampPan,
    suspendView,
    restoreView,
    clearError,
    revealCurrent,
    deleteCurrent,
    stopSlideshow,
    toggleSlideshow,
    toggleAnimation,
    togglePlayback,
    frameUrl,
    close,
  };
});
