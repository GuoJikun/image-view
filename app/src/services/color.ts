/**
 * 显示目标色彩空间的探测。
 *
 * 画布只能声明两个空间（`CanvasRenderingContext2DSettings.colorSpace` 的取值只有
 * `srgb` 与 `display-p3`），所以这里只做两档判断：屏幕是否广色域、引擎是否真的
 * 给了 P3 画布。两者都成立才切到 P3，否则一律 sRGB。
 *
 * 探测是同步的，并且在模块加载时执行一次：`ImageCanvas` 首次拿 context 之前就
 * 必须先知道目标空间，之后再想改只能重建画布元素。
 */
import type { ColorTarget } from "../types/ipc";

/** 不支持的引擎会静默忽略 `colorSpace`，必须回读实际值确认。 */
function engineSupportsP3Canvas(): boolean {
  const context = document
    .createElement("canvas")
    .getContext("2d", { colorSpace: "display-p3" });
  const attributes = context?.getContextAttributes?.() as
    | { colorSpace?: string }
    | null
    | undefined;
  return attributes?.colorSpace === "display-p3";
}

function detect(): ColorTarget {
  const wideGamut = window.matchMedia?.("(color-gamut: p3)").matches ?? false;
  return wideGamut && engineSupportsP3Canvas() ? "display-p3" : "srgb";
}

/** 本次会话的目标空间：画布创建与后端解码都读这一个值。 */
export const colorTarget: ColorTarget = detect();

/** 空间名称是标准专有名词，两种语言下写法一致，不需要进语言文件。 */
export function targetLabel(target: ColorTarget | null | undefined): string {
  return target === "display-p3" ? "Display P3" : "sRGB";
}

/**
 * 监听显示能力变化（窗口拖到另一块屏、HDR 开关）。
 *
 * 回调里由调用方负责通知后端并重新请求当前图片。画布的 `colorSpace` 在首次
 * `getContext` 时就固定了，所以切换只在下次启动才真正生效；但颜色始终正确——
 * 预览 PNG 带着 iCCP 标签，画布空间与标签不一致时浏览器会自己换算。
 */
export function onColorTargetChange(handler: (next: ColorTarget) => void): () => void {
  const query = window.matchMedia("(color-gamut: p3)");
  const listener = (): void => {
    const next = detect();
    if (next !== colorTarget) handler(next);
  };
  query.addEventListener("change", listener);
  return () => query.removeEventListener("change", listener);
}
