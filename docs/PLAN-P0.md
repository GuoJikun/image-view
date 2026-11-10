# P0 实现计划与验收：Rust 图片查看器（v1.0 / v1.1）

| 项目 | 内容 |
| --- | --- |
| 阶段 | P0（已完成基线） |
| 文档版本 | v1.1（已完成） |
| 日期 | 2026-11-10 |
| 目标平台 | Windows 10/11 x64 优先，代码保持 macOS / Linux 可编译 |
| 技术栈 | Rust 2024 + eframe/egui 0.37 + image 0.25 |
| 预估工期 | 5 ~ 6.5 个工作日（单人），v1.1 前两项另计 0.5 天 |
| 当前状态 | P0 与 v1.1 全部完成；验证记录见 §9.5 ~ §9.7 |
| 后续计划 | [ROADMAP.md](ROADMAP.md)（总览）· [PLAN-P1.md](PLAN-P1.md) · [PLAN-P2.md](PLAN-P2.md) · [PLAN-P3.md](PLAN-P3.md) |

---

## 目录

1. [项目目标与范围](#1-项目目标与范围)
2. [技术选型](#2-技术选型)
3. [功能需求](#3-功能需求)
4. [架构设计](#4-架构设计)
5. [关键设计详述](#5-关键设计详述)
6. [项目结构](#6-项目结构)
7. [Cargo.toml 草案](#7-cargotoml-草案)
8. [里程碑与任务拆解](#8-里程碑与任务拆解)
9. [测试计划](#9-测试计划)
10. [性能预算](#10-性能预算)
11. [风险与应对](#11-风险与应对)
12. [后续路线](#12-后续路线)
13. [待确认决策](#13-待确认决策)
14. [附录](#14-附录)

---

## 1. 项目目标与范围

### 1.1 一句话定义

一个轻量、原生、启动快的桌面图片查看器：打开一张图，能缩放、平移、在同目录内翻页，仅此而已。

### 1.2 MVP（P0）范围

- 打开图片：文件对话框、命令行参数、拖放文件。
- 正确显示：适配窗口 / 100% 物理像素、深色背景、透明图棋盘格。
- 交互浏览：鼠标滚轮缩放（以光标为锚点）、拖拽平移、双击切换适配/100%。
- 同目录导航：上一张 / 下一张，自然排序，首尾不循环。
- 状态信息：文件名、原始尺寸、当前缩放百分比、当前序号。
- 健壮性：损坏文件、无权限、超大图不崩溃，错误以非阻塞横幅提示。
- 大图安全：解码前检查尺寸，超过阈值时降采样后再上传纹理。

### 1.3 明确不做（v1）

| 不做的事 | 原因 |
| --- | --- |
| 图片编辑（裁剪/调色/旋转保存） | 查看器定位，避免范围膨胀 |
| 相册/图库管理、标签、搜索 | 需要数据库与索引，属于另一个产品 |
| GIF/WebP 动画播放 | 第一版只显示首帧；动画需要逐帧解码与计时器 |
| HEIC/AVIF/RAW | `image` 默认能力不覆盖，需额外原生依赖 |
| ICC 色彩管理、HDR | 需要 `lcms2` 等，v1 接受偏色 |
| 多标签、多窗口、对比模式 | 交互复杂度高，非基础功能 |
| 安装包与文件关联 | 先产出可运行的 exe，后续再做安装器 |

### 1.4 验收标准（Definition of Done）

1. `cargo build --release` 产出单个 exe，双击可启动；冷启动到首帧 ≤ 600 ms（SSD）。
2. 双击 exe 或拖入 24MP JPEG，图片 ≤ 600 ms 可见，像素方向与解码结果一致（EXIF 方向校正在 v1.1 已实现）。
3. 打开 100MP 图片时 UI 不冻结、不 OOM；显示 8K 以内降采样结果并提示“已降采样”。
4. 缩放/平移在 24MP 图片上保持 60 fps（常规窗口尺寸）。
5. 支持的扩展名（见 §5.1）均能打开；损坏文件给出可读错误且不 panic。
6. 中文路径、空格路径、长路径、UNC 路径正常。
7. 连续快速按 `→` 时只显示最终一张，不出现旧图覆盖新图。
8. 高 DPI（125% / 150% / 200%）下 100% 缩放为图像像素与屏幕物理像素 1:1。
9. `cargo fmt --check`、`cargo clippy -D warnings`、`cargo test` 全部通过。

---

## 2. 技术选型

### 2.1 候选方案对比

| 方案 | 纯 Rust | 上手成本 | 图像纹理控制力 | 启动/体积 | 结论 |
| --- | --- | --- | --- | --- | --- |
| **eframe / egui 0.37** | 是 | 低（即时模式，无状态绑定） | 高（可直接拿 `TextureHandle` + `Painter`） | 启动快，exe 约 10~20 MB | **采用** |
| iced | 是 | 中（Elm 架构，消息/订阅） | 中（自定义 widget 稍绕） | 相近 | 备选 |
| Slint | 是（DSL） | 中（需学 .slint 语法） | 中 | 相近 | 适合表单类 UI |
| Tauri v2 + Web 前端 | 否（Rust + JS） | 高（双栈） | 低（Canvas 自己写） | WebView 运行时，体积大 | 不适合轻量看图 |
| 原生 Win32（windows crate） | 是 | 很高 | 高 | 最小 | 开发效率太低 |

### 2.2 结论与理由

采用 **eframe 0.37 + egui 0.37**：

- 即时模式 UI 对“一个画布 + 两条工具栏”这种界面几乎零样板代码。
- 可以直接控制纹理与绘制矩形：缩放、平移、双线性过滤都由自己决定，不受内置图片组件限制。
- `eframe` 默认使用 wgpu 渲染，并默认带系统字体回退（DirectWrite），中文文件名/界面开箱即用。
- 编译产物是单个原生 exe，无运行时依赖。

配套 crate：

| crate | 版本 | 用途 | 备注 |
| --- | --- | --- | --- |
| `eframe` | 0.37 | 窗口、事件循环、GPU 渲染 | 默认特性含 `wgpu`、`system_fonts` |
| `egui` | 0.37 | 立即模式 UI | 必须与 eframe 版本一致 |
| `image` | 0.25.10 | 解码、缩放、像素格式转换 | 关闭默认特性，只开需要的格式以缩短编译 |
| `rfd` | 0.18 | 原生文件选择对话框 | Windows 使用 IFileDialog，阻塞式 |
| `natord` | 1.0.9 | 文件名自然排序（`2` 在 `10` 前） | 体积极小 |
| `thiserror` | 2.0.21 | 加载错误类型 | 库内错误 |
| `anyhow` | 1.0.104 | 应用层错误汇总 | 仅边界使用 |
| `log` / `env_logger` | 0.4.35 / 0.11.11 | 诊断日志 | 默认 `warn`，可用 `RUST_LOG` 调整 |

> 版本为 2026-11-10 查询到的最新稳定版。提交 `Cargo.lock`；升级 egui 时按 §14 的 API 检查清单回归。

---

## 3. 功能需求

### 3.1 P0 功能清单

| ID | 功能 | 行为说明 | 验收标准 |
| --- | --- | --- | --- |
| F-01 | 打开文件 | `Ctrl+O` 或工具栏按钮，原生对话框，过滤器为支持的图片扩展名 | 选中后 1s 内显示 |
| F-02 | 命令行参数 | `image-view.exe path\to\img.jpg` | 启动即显示；参数非法时提示并打开空界面 |
| F-03 | 拖放打开 | 从资源管理器拖入窗口 | 打开第一个图片文件；多文件取第一个 |
| F-04 | 适配显示 | 新图片默认 Fit：完整可见，且不放大超过 100% | 任意窗口尺寸不裁切、不变形 |
| F-05 | 缩放 | 滚轮 / 触控板捏合 / `+` `-` / 工具栏按钮；以光标为锚点；范围 1% ~ 3200% | 光标下的图像点缩放前后不漂移 |
| F-06 | 平移 | 按住左键拖拽；图像大于窗口时生效 | 松手后不越界，图像不会“飞走” |
| F-07 | 视图模式 | 双击、`F`、`1`、`0`、工具栏按钮切换 Fit / 100% | 100% 时 1 图像像素 = 1 屏幕物理像素 |
| F-08 | 目录导航 | 扫描当前文件所在目录，按自然顺序上一张/下一张；`←` `→`；首尾禁用 | `img2` 排在 `img10` 前；不循环 |
| F-09 | 状态栏 | 文件名、原始尺寸、当前缩放、序号、加载状态 | 实时更新，无抖动 |
| F-10 | 窗口标题 | `文件名 — 尺寸 (缩放) — 图片查看器` | 仅在变化时更新，不刷帧 |
| F-11 | 错误提示 | 可关闭的红色横幅，显示原因；日志保留详情 | 不弹模态框，不 panic |
| F-12 | 异步加载 | 解码在工作线程；加载中显示 spinner；旧请求结果丢弃 | 快速连按 `→` 只显示最后一张 |
| F-13 | 大图降采样 | 超过纹理/内存上限时缩放，并在状态栏标记 | 100MP 图片可打开且内存受控 |
| F-14 | 透明背景 | 图像区域绘制浅色棋盘格 | PNG 透明区可见 |

### 3.2 P1（v1.1，已完成）

- [x] EXIF 方向自动校正（`image` 内置 `ImageDecoder::orientation`，无需额外依赖）。
- [x] 顺时针/逆时针旋转 90°（`R` / `Shift+R`，几何旋转，不重传纹理）。
- [x] 全屏（`F11`）已提前实现；无边框沉浸模式待做。
- [x] 幻灯片（间隔 3s，空格开始/暂停；到最后一张自动停止）。
- [x] 相邻图片预加载与 LRU 纹理缓存（3 张，总像素 ≤ 72M）。

v1.1 未完成的体验项（窗口状态持久化、Mipmap、回收站/复制路径、文件夹位置）已并入 [`PLAN-P1.md`](PLAN-P1.md) 的 P1-M1 / P1-M5。

### 3.3 P2（远期，已迁出）

远期需求不再在本文维护，按阶段拆分到独立文件：[PLAN-P1.md](PLAN-P1.md)（v2.0）、[PLAN-P2.md](PLAN-P2.md)（v2.x）、[PLAN-P3.md](PLAN-P3.md)（v3+，新增）。

### 3.4 快捷键表（v1）

| 快捷键 | 功能 | 备注 |
| --- | --- | --- |
| `Ctrl+O` | 打开文件 | |
| `←` / `→` | 上一张 / 下一张 | 到达首尾时禁用 |
| `+` / `=` | 放大 | 以画布中心为锚点（键盘操作） |
| `-` | 缩小 | 同上 |
| `0` | 适配窗口 | 双击画布也可切换 |
| `1` | 100% | |
| `F` | 切换适配 / 100% | |
| `F11` | 全屏 | 已提前实现（原 P1） |
| `R` | 顺时针旋转 90° | v1.1 已实现 |
| `Shift+R` | 逆时针旋转 90° | v1.1 已实现 |
| `Space` | 幻灯片开始 / 停止 | v1.1 已实现；到最后一张自动停止 |
| `Esc` | 退出全屏 / 关闭帮助 | |
| `Ctrl+W` / `Alt+F4` | 关闭窗口 | 交给系统 |
| `Ctrl++` / `Ctrl+-` | 缩放 | 与浏览器习惯一致，P1 可选 |

鼠标：

- 滚轮：缩放（图像查看器惯例）；触控板双指捏合：缩放。
- 左键拖拽：平移。
- 双击：Fit ⇄ 100%。
- 中键：无操作（保留给 P1 的“按住临时平移”）。

---

## 4. 架构设计

### 4.1 模块总览

```text
┌─────────────────────────────────────────────────────────────┐
│ main.rs            入口：CLI、NativeOptions、启动 eframe     │
├─────────────────────────────────────────────────────────────┤
│ app.rs             ImageViewerApp：状态机、update 编排       │
│  ├─ ui.rs          顶部工具栏 / 底部状态栏 / 错误横幅 / 帮助 │
│  ├─ viewer.rs      ViewState（zoom/pan/mode）+ 画布绘制      │
│  ├─ document.rs    LoadedImage（纹理、尺寸、路径）           │
│  ├─ cache.rs       已解码纹理的 LRU 缓存（3 张 / 72M 像素）  │
│  ├─ loader.rs      解码线程、降采样、取消、预加载、错误      │
│  └─ folder.rs      目录扫描、自然排序、上一张/下一张         │
└─────────────────────────────────────────────────────────────┘
             │ request(path, gen, purpose)  ▲ Result(gen, purpose, image)
             ▼                             │
      ┌──────────────────────────────────────────┐
      │ decoder 工作线程（std::thread + mpsc）    │
      │ 优先级队列：Open > Preload；latest-wins  │
      │ ImageReader → 尺寸检查 → 解码 → 降采样   │
      └──────────────────────────────────────────┘
```

### 4.2 线程模型

- **UI 主线程**：所有 egui 状态、`TextureHandle` 创建、纹理上传、窗口标题更新。
- **decoder 线程（1 个）**：只做纯 CPU 工作，返回 `Vec<u8>` RGBA 数据，不接触任何 GPU/egui 纹理对象。
- 通信：`std::sync::mpsc` 双向通道；decoder 完成后调用 `ctx.request_repaint()` 唤醒 UI。
- 取消：递增 `generation`；只接受与当前 `generation` 相同的结果。worker 每次解码前先排空请求队列：新的 `Open` 覆盖所有待办（latest-wins），只有 `Preload` 时按路径去重后追加，避免快速翻页时积压。
- 优先级：队列中始终先取 `Open`，预加载不会挡住翻页；缓存命中时 `cancel_pending()` 只推进代次，让在途结果过期，不再发解码请求。
- 预加载结果不带代次校验（仅填充缓存），这样快速翻页时已解码完成的邻居仍可复用；主线程只接受当前目录内、且尚未缓存的路径。
- 不引入 async runtime：一个线程 + 两个通道足够，依赖更少、行为更可预测。

### 4.3 核心数据类型

```rust
// app.rs
pub struct ImageViewerApp {
    pub folder: FolderNav,             // 当前目录与索引
    pub image: Option<LoadedImage>,    // 当前纹理
    pub view: ViewState,               // 缩放/平移
    pub loader: Loader,                // 解码线程句柄
    pub generation: u64,               // 当前有效请求代次
    pub pending: Option<PathBuf>,      // 正在加载的路径
    pub error: Option<String>,         // 待展示错误
    pub title_cache: String,           // 避免每帧发 ViewportCommand
    pub cache: ImageCache,             // 最近使用的 3 张纹理
    pub slideshow: Option<Slideshow>,  // 幻灯片计时（next_at）
}

// document.rs
pub struct LoadedImage {
    pub path: PathBuf,
    pub texture: egui::TextureHandle,
    pub display_size: [usize; 2],      // 纹理尺寸（可能已降采样）
    pub source_size: [usize; 2],       // 原图尺寸
    pub downscaled: bool,
}

// viewer.rs
pub struct ViewState {
    pub zoom: f32,        // 物理像素 / 纹理像素；1.0 = 100%
    pub pan: egui::Vec2,  // 图像中心相对画布中心的偏移（逻辑点）
    pub mode: ViewMode,   // Fit | Manual
}

// folder.rs
pub struct FolderNav {
    pub dir: PathBuf,
    pub files: Vec<PathBuf>,
    pub index: Option<usize>,
}
```

### 4.4 一次“打开图片”的数据流

1. 用户触发打开（对话框 / CLI / 拖放 / 翻页）。
2. `app` 校验路径，更新 `folder`（必要时重新扫描目录并定位索引）。
3. `let gen = loader.request(path)` 把请求发给 decoder 线程并拿到代次；`self.generation = gen`，`pending = Some(path)`。
4. decoder：`ImageReader::open` → 先读尺寸（防解压炸弹）→ `decode()` → 必要时 `resize_exact` → 输出 RGBA8 + 尺寸 + 原尺寸。
5. worker 检查该请求是否仍为最新；发送结果并 `request_repaint()`。
6. 主线程 `poll_loader()`：丢弃过期代次；把 `ColorImage` 上传为纹理；替换 `image`；`view.reset_fit()`；清空 `pending`。
7. 通知 UI 更新标题与状态栏；错误则写入 `error` 横幅。
8. 下一帧绘制：Fit 模式下按画布实时计算缩放。

### 4.5 egui 使用约束

- 纹理只在主线程创建；worker 不得持有 `TextureHandle`。
- 空闲时不要 `request_repaint()`（保持 0% CPU）；仅在加载完成、动画、拖动时重绘。
- 不在 `update` 中做磁盘 I/O、解码、`read_dir`（打开动作一次性执行，其余在 worker）。
- 窗口标题只在字符串变化时发送 `ViewportCommand::Title`。
- 画布用 `ui.allocate_rect(rect, Sense::click_and_drag())` 获取交互响应，避免与面板布局打架。

---

## 5. 关键设计详述

### 5.1 图像解码与纹理

#### 支持的格式（v1）

| 扩展名 | 解码器 | 说明 |
| --- | --- | --- |
| `.png` | `png` | 含透明通道 |
| `.jpg` / `.jpeg` | `zune-jpeg` | 不支持 ICC/CMYK 正确色彩（后续 P2） |
| `.gif` | `gif` | 仅首帧 |
| `.webp` | `image-webp` | 仅首帧 |
| `.bmp` / `.ico` / `.tga` / `.pnm` / `.qoi` / `.dds` | 内置 | 基础兼容 |
| `.tif` / `.tiff` | `tiff` | 16-bit 会转 8-bit |
| `.hdr` / `.exr` | 内置 | 转为 8-bit，仅预览 |

解码器特性在 `Cargo.toml` 中显式开启，关闭 `avif` / `ff` 以减少编译时间与体积。

#### 解码流程

```rust
fn decode(path: &Path, caps: &Caps) -> Result<Decoded, LoadError> {
    let mut reader = image::ImageReader::open(path)?
        .with_guessed_format()?;

    // 1) 先读头部尺寸，拒绝解压炸弹
    let (w, h) = reader.into_dimensions()?;
    if (w as u64) * (h as u64) > caps.max_source_pixels {
        return Err(LoadError::TooLarge { w, h });
    }

    // 2) 重新打开并设置内存上限（防止解码阶段超大分配）
    let mut reader = image::ImageReader::open(path)?.with_guessed_format()?;
    reader.limits(caps.image_limits()); // image::Limits：限制宽高与分配字节数
    let img = reader.decode()?;

    // 3) 必要时降采样（在 worker 中完成，避免主线程卡顿）
    let source_size = [img.width() as usize, img.height() as usize];
    let target = target_size(source_size, caps.max_side, caps.max_pixels);
    let img = if target != source_size {
        let filter = if source_size[0] * source_size[1] > 100_000_000 {
            image::imageops::FilterType::Triangle   // 超大图优先速度
        } else {
            image::imageops::FilterType::Lanczos3   // 常规图优先质量
        };
        img.resize_exact(target[0] as u32, target[1] as u32, filter)
    } else {
        img
    };

    // 4) 统一为 RGBA8，交给主线程上传
    let rgba = img.to_rgba8().into_raw();
    Ok(Decoded { size: target, source_size, rgba })
}
```

#### 大图与安全阈值

| 常量 | 建议值 | 依据 |
| --- | --- | --- |
| `MAX_SOURCE_PIXELS` | 250 MP | 超过则拒绝解码，防止恶意/异常文件耗尽内存（实测 300MP 文件即被拒绝） |
| `MAX_TEXTURE_SIDE` | `min(8192, ctx.input(|i| i.max_texture_side))` | 兼顾显存与老 GPU 的 8192 上限；首帧前若为 0 则用 8192 兜底 |
| `MAX_TEXTURE_PIXELS` | 40 MP | 8192×4883 ≈ 40MP，RGBA8 约 160 MB |
| `MAX_IMAGE_BYTES`（image Limits） | 512 MB | 覆盖解码中间缓冲 |

超过阈值时：按 `min(max_side/最长边, sqrt(max_pixels/总像素))` 计算比例，等比缩小；状态栏显示“已降采样 8192×4096（原图 20000×10000）”。

> 关于“100%”的语义：降采样过的图片，100% 是相对**纹理**的；状态栏同时显示原图尺寸，避免误解。

#### 内存估算

- 24MP（6000×4000）RGBA8 = 96 MB；上传纹理后 egui 会释放 CPU 侧副本，峰值约 2 倍。
- 40MP 上限 = 160 MB 纹理 + 上传瞬时 160 MB；对 8 GB 内存机器可接受。
- 解码后的 CPU 缓冲在 `load_texture` 后立即 drop，不常驻。

#### EXIF 方向（v1.1 已实现）

- 使用 `image` 0.25 内置的 `ImageDecoder::orientation()` 读取 `Orientation` (0x0112)，无需 `kamadak-exif`。
- 只有 JPEG / TIFF / WebP 的解码器提供 EXIF；其他格式返回 `NoTransforms`，按原始像素显示。
- 在 worker 中先 `DynamicImage::apply_orientation`，再计算尺寸、降采样与转 RGBA8，保证降采样基于最终方向。

#### 动画

v1 统一取首帧。GIF/WebP 的动画播放需要保留帧缓冲与计时器，列入 P2。

### 5.2 视图变换

#### 坐标与定义

- 纹理坐标：以纹理像素为单位，原点左上。
- 画布坐标：egui 逻辑点（point）；`ppp = ctx.pixels_per_point()` 是物理像素/逻辑点。
- 状态：`zoom` = 物理像素 / 纹理像素（100% 即 1.0）；`pan` = 图像中心相对画布中心的偏移（point）。
- 派生：`scale = zoom / ppp`（point / 纹理像素）。

绘制矩形：

```text
image_center = canvas.center() + pan
draw_size    = displayed_size(texture_size, rotation) * scale
draw_rect    = Rect::from_center_size(image_center, draw_size)
```

`displayed_size` 在奇数个 90° 旋转时交换宽高；`rotation == 0` 时退化为 `texture_size`。

这样在任意 DPI 下，100% 都是 1 纹理像素 = 1 物理像素。

#### 适配（Fit）

```rust
fn fit_zoom(view: Vec2, image: Vec2, ppp: f32) -> f32 {
    // view 与 image 都是物理像素口径后比较，且不放大超过 100%
    let z = (view.x * ppp / image.x).min(view.y * ppp / image.y);
    z.min(1.0)
}
```

Fit 模式每帧按当前画布尺寸重算，窗口 resize 自动跟随，`pan = 0`。

#### 以光标为锚点缩放（核心）

设画布中心为原点，光标位置 `c`，旧缩放 `s0`，新缩放 `s1`：

```text
光标下的纹理点（相对图像中心）: p = (c - pan) / s0
缩放后要让 p 仍在光标下:        pan' = c - p * s1
```

代码：

```rust
pub fn zoom_at(
    &mut self,
    cursor: Vec2,      // 相对画布中心
    image: Vec2,
    view: Vec2,
    factor: f32,
    ppp: f32,
) {
    let s0 = self.zoom / ppp;
    let p = (cursor - self.pan) / s0;
    self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
    let s1 = self.zoom / ppp;
    self.pan = cursor - p * s1;
    self.mode = ViewMode::Manual;
    self.clamp_pan(image, view, ppp);
}
```

- `MIN_ZOOM = 0.01`（1%），`MAX_ZOOM = 32.0`（3200%）。
- 滚轮因子：`(scroll_y * 0.0015).exp()`；触控板捏合直接乘 `i.zoom_delta()`；两者不同时生效，避免叠加。
- 键盘 `+`/`-`：以画布中心为锚点，因子 1.25 / 0.8。

#### 平移边界

```rust
fn clamp_pan(&mut self, image: Vec2, view: Vec2, ppp: f32) {
    let disp = image * (self.zoom / ppp);
    let limit = ((disp - view) / 2.0).max(Vec2::ZERO);
    self.pan.x = self.pan.x.clamp(-limit.x, limit.x);
    self.pan.y = self.pan.y.clamp(-limit.y, limit.y);
}
```

- 图像小于画布时 `limit = 0`，自动居中。
- 拖拽时实时钳制，避免松手“回弹”。

#### 模式切换行为

| 触发 | 行为 |
| --- | --- |
| 打开新图 | 重置为 Fit，`pan = 0` |
| 窗口 resize | Fit 模式重算；Manual 模式保持 zoom，重新钳制 pan |
| 双击 | Fit ⇄ 100%（以光标为锚点过渡，或直接居中，v1 直接居中更简单） |
| 滚轮 / `+` / `-` | 进入 Manual，保留当前 zoom 作为基准 |
| 切换 DPI（跨显示器） | `zoom` 是物理口径，几何自动正确；重算 Fit 或钳制 pan |

#### 旋转（v1.1 已实现）

采用**几何旋转**：`egui::Mesh` 四边形 + 旋转 UV 表，只改变采样方式，不重传纹理。

- `ViewState.rotation` 记录顺时针 90° 的次数（0~3），只影响显示；换图时随 `ViewState::default()` 重置。
- `displayed_size()` 在奇数步时交换宽高；Fit、缩放钳制、平移钳制都以旋转后的尺寸为准。
- 旋转后 `pan` 归零，避免图像跳出画布；`R` 顺时针、`Shift+R` 逆时针。
- 屏幕四角对应的纹理 UV 表：0° `(0,0)(1,0)(1,1)(0,1)`；90° `(0,1)(0,0)(1,0)(1,1)`；180° `(1,1)(0,1)(0,0)(1,0)`；270° `(1,0)(1,1)(0,1)(0,0)`。
- 曾考虑 CPU 旋转像素后重传纹理，但 24MP 图一次旋转有几十毫秒卡顿，且需要额外的纹理生命周期管理，故不采用。

### 5.3 输入处理顺序

每帧顺序固定，避免同一事件被重复消费：

1. `handle_shortcuts(ctx)`：`Ctrl+O`、`←/→`、`+/-`、`0/1/F`、`R` / `Shift+R`、`Esc`。使用 `consume_key`，避免长按重复触发翻页；`consume_key` 采用逻辑修饰键匹配（多余的 Shift 不算冲突），因此共享逻辑键的 `Shift+R` 必须先于 `R` 匹配。
2. `poll_loader(ctx)`：接收解码结果，过期结果丢弃。
3. 画布绘制与指针交互：
   - `resp.hovered()` 时处理滚轮/捏合缩放；只有指针在画布上才缩放。
   - `resp.dragged()` 时 `pan += resp.drag_delta()`，并钳制；`drag_started` 时进入 Manual。
   - `resp.double_clicked()` 切换模式。
4. 面板绘制（顶部、底部）。
5. 标题更新：`logic()` 中先用上一帧画布尺寸重算 Fit 缩放，保证标题/工具栏显示与画布一致。

细节：

- 滚轮滚动在 egui 中可能同时产生 `smooth_scroll_delta` 与 `zoom_delta`（Ctrl+滚轮）。约定：`zoom_delta != 1.0` 时只用捏合，否则用滚轮；不同时叠加。
- 拖拽阈值交给 egui 默认（几像素），避免单击误触发平移。
- 文本输入框在 v1 中不存在，因此快捷键无需 `wants_keyboard_input` 检查；P1 加搜索框时需补上。

### 5.4 目录导航

```rust
fn scan(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.is_file() && is_supported_image(&path) {
            files.push(path);
        }
    }

    files.sort_by(|a, b| {
        let an = a.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        let bn = b.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
        natord::compare(&an, &bn).then_with(|| a.cmp(b))
    });
    files
}
```

规则：

- 只扫描当前文件所在目录，不递归、不跟随符号链接目录。
- 扩展名匹配大小写不敏感；与 §5.1 列表一致。
- 自然排序，不区分大小写；同名不同大小写用完整路径兜底，保证排序稳定。
- 当前文件不在列表中（例如通过对话框选了不支持的扩展名但能解码）时，临时插入并定位。
- 首张/末张时按钮与快捷键禁用（不循环）。
- 目录扫描在打开动作中同步执行；2 万文件实测通常 < 100 ms，可接受。若后续要支持超大目录，改为 worker。

### 5.5 UI 布局

```text
┌───────────────────────────────────────────────────────────────┐
│ [打开] [◀] [▶]   [适配] [100%] [−] 67% [+]  [幻灯片]  [帮助]  │ 顶部工具栏
├───────────────────────────────────────────────────────────────┤
│                                                               │
│                      图像画布（可拖拽）                       │ 中央画布
│                                                               │
├───────────────────────────────────────────────────────────────┤
│ photo.jpg  6000×4000  67%  3/128  [已降采样] [幻灯片中] [加载…]│ 底部状态栏
└───────────────────────────────────────────────────────────────┘
```

- 顶部：`egui::Panel::top`，按钮 + 缩放百分比（v1 只读）。
- 底部：`egui::Panel::bottom`，左对齐状态文本，右侧错误/加载指示。
- 画布：`egui::CentralPanel` + 无边框 `Frame`，`Color32::from_gray(18)` 背景。
- 棋盘格：P0 实现（F-14）；以图像绘制矩形为范围，8 px 格子，两种灰色交替。注意棋盘格需绘制在图像下方、背景之上。
- 无图片时：画布中央显示“拖入图片，或按 Ctrl+O 打开”。
- 错误横幅：顶部工具栏下方一条红色背景，右侧“×”关闭；新的错误替换旧的。
- 帮助：`?` 按钮打开一个简单窗口，列出快捷键（P1 可省略）。
- 主题：`ctx.set_visuals(Visuals::dark())`，强调色用蓝色；中文字体依赖 eframe 默认的系统字体回退。

### 5.6 错误处理与日志

```rust
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("无法读取文件：{0}")]
    Io(#[from] std::io::Error),
    #[error("无法识别图片格式")]
    UnsupportedFormat,
    #[error("图片已损坏或格式不受支持：{0}")]
    Decode(#[from] image::ImageError),
    #[error("图片过大（{w}×{h}），已超过安全上限")]
    TooLarge { w: u32, h: u32 },
    #[error("解码超出内存限制")]
    OutOfMemory,
    /// 预加载遇到大图时主动放弃；仅写 debug 日志，不展示横幅。
    #[error("预加载已跳过（原图 {w}×{h} 过大）")]
    PreloadSkipped { w: u32, h: u32 },
}
```

- decoder 返回 `Result<Decoded, LoadError>`；主线程转成中文横幅。
- 日志：`log::error!` 记录路径与原始错误；`RUST_LOG=image_view=debug` 可看耗时。
- 不 panic：所有 `unwrap` 仅用于“构造时不可能失败”的场景，并用注释说明。
- 文件不存在/被移动：翻页时跳过失效项，不重新扫描目录；新增文件需重新打开目录才能看到。

### 5.7 窗口、DPI、字体与主题

- 默认窗口 1100×760，最小 480×320；标题栏使用系统装饰。
- 开启 `with_drag_and_drop(true)`；Windows 下拖放由 winit 提供。
- `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` 隐藏控制台。
- 图标：`assets/icon.png` 用 `image` 解码为 `egui::IconData`，`ViewportBuilder::with_icon` 设置（P1 可加 `.ico` 资源）。
- 字体：eframe 0.37 默认启用 `system_fonts`，缺失字形会通过 DirectWrite 回退到微软雅黑等系统字体；需专项测试中文文件名与中文 UI。若目标机器精简系统无中文字体，P1 改为内嵌 Noto Sans SC 子集（约 5~10 MB）。
- 主题：深色；后续可读取系统主题（P2）。
- 跨平台：路径用 `PathBuf`/`OsStr`；`rfd` 提供各平台原生对话框；Windows 专属代码仅图标/子系统属性。

### 5.8 命令行接口

```text
image-view [PATH]

参数:
  PATH    要打开的图片文件或目录（目录则打开其中第一张图片）

选项:
  -h, --help       显示帮助
  -V, --version    显示版本
```

- 仅在启动时解析一次，使用 `std::env::args_os()` 保留非 UTF-8 路径。
- 无参数：空界面。
- 参数是目录：扫描并打开第一张；为空则提示。
- 参数不存在：显示错误横幅，不退出。

### 5.9 幻灯片与预加载缓存（v1.1）

**幻灯片**

- 状态：`Slideshow { next_at: Instant }`，固定间隔 3s；`Space` 或工具栏“幻灯片”按钮开始/停止。
- 驱动：`logic()` 中的 `tick_slideshow()`；未到点用 `ctx.request_repaint_after(next_at - now)` 定时，不轮询、不空转。
- 加载中（`pending.is_some()`）每 250ms 重试，避免把翻页请求堆积起来。
- 手动 `←`/`→` 重置计时；显式打开文件/目录、加载失败时停止。
- 到最后一张自动停止（与“翻页不循环”一致），不回卷到第一张。

**预加载与 LRU 缓存**

- 每次换图后预加载前后邻居（跳过已缓存、不存在的项）；预加载请求标记 `Purpose::Preload`，worker 始终优先处理 `Purpose::Open`。
- 预加载限流：原图超过 30 MP 时返回 `LoadError::PreloadSkipped`，只写 debug 日志、不弹横幅；避免 100MP 图片在后台解码拖慢翻页。
- 请求命中缓存时直接显示、不排队解码；用 `Loader::cancel_pending()` 推进代次，使在途 `Open` 结果过期，避免旧图覆盖新图。
- 缓存：`ImageCache` 最多 3 条、总像素 ≤ 72M（约 288 MB RGBA8，可容纳 3 张 24MP）；命中移到队尾（LRU），当前显示的图片永不淘汰。
- 缓存条目持有 `TextureHandle`（`Clone` 共享同一纹理），回翻不重传；淘汰时随最后一个句柄释放。
- 预加载结果只入缓存、不校验代次，快速翻页时已解码完成的邻居仍可复用；主线程只接受当前目录内且尚未缓存的路径。

---

## 6. 项目结构

```text
image-view/
├─ Cargo.toml
├─ Cargo.lock
├─ .gitignore
├─ assets/
│  └─ icon.png
├─ docs/
│  ├─ ROADMAP.md              # 阶段进度总览与里程碑看板
│  ├─ PLAN-P0.md              # 本文档：P0 基线（v1.0 / v1.1）
│  ├─ PLAN-P1.md              # P1：v2.0 格式、色彩、浏览与产品化
│  ├─ PLAN-P2.md              # P2：v2.x 专业能力与架构增强
│  └─ PLAN-P3.md              # P3：v3+ 新增长期方向
├─ src/
│  ├─ main.rs                 # 入口、CLI、窗口选项、图标
│  ├─ app.rs                  # App 状态与 update 编排（~300 行）
│  ├─ ui.rs                   # 顶部/底部面板、错误横幅、帮助（~150 行）
│  ├─ viewer.rs               # ViewState、变换数学、画布绘制、指针交互（~300 行）
│  ├─ document.rs             # LoadedImage、标题/状态文本（~80 行）
│  ├─ cache.rs                # LRU 纹理缓存（~80 行）
│  ├─ loader.rs               # 解码线程、阈值、降采样、预加载、错误（~350 行）
│  └─ folder.rs               # 目录扫描、排序、前后导航（~120 行）
└─ tests/
   └─ fixtures/               # 小型测试图（PNG/JPEG/GIF/损坏文件）
```

预估代码量：**约 1,300~1,600 行**（含单元测试约 2,000 行）。

---

## 7. Cargo.toml 草案

```toml
[package]
name = "image-view"
version = "0.1.0"
edition = "2024"
rust-version = "1.95"          # eframe 0.37 的 MSRV
description = "轻量级 Rust 桌面图片查看器"

[dependencies]
eframe = "0.37"                # 默认启用 wgpu 渲染 + 系统字体回退
egui = "0.37"                  # 与 eframe 保持同一版本
image = { version = "0.25.10", default-features = false, features = [
    "png", "jpeg", "gif", "webp", "bmp", "tiff", "ico",
    "qoi", "tga", "pnm", "hdr", "exr", "dds", "rayon",
] }
rfd = "0.18"
natord = "1.0"
thiserror = "2"
anyhow = "1"
log = "0.4"
env_logger = "0.11"

[profile.release]
lto = "thin"
codegen-units = 1
strip = "symbols"

[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 2                  # 让依赖（image/eframe）在 debug 下也能用
```

说明：

- **渲染后端**：eframe 0.37 默认使用 wgpu。若遇到老显卡驱动问题，可加 `features = ["glow"]` 并在 `NativeOptions.renderer` 切换。
- **不用 `egui_extras`**：其图片加载器偏向“按 URI 显示”，不便控制缩放、降采样与旋转；这里自行解码更直接。
- **`image` 格式裁剪**：不启用 `avif`/`ff`；如需 AVIF，后续单独评估 `avif-native`。
- **单文件 exe**：MSVC 目标默认动态链接 VC 运行时；如需“拷贝即用”，可加 `.cargo/config.toml`：

  ```toml
  [target.x86_64-pc-windows-msvc]
  rustflags = ["-C", "target-feature=+crt-static"]
  ```

  发布前需验证 wgpu/rfd 在静态 CRT 下正常。
- **`rust-toolchain.toml` 可选**：

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

---

## 8. 里程碑与任务拆解

总览：

| 里程碑 | 目标 | 预估 | 累计 |
| --- | --- | --- | --- |
| M0 | 脚手架与空窗口 | 0.5 d | 0.5 d |
| M1 | 打开并显示图片 | 1 d | 1.5 d |
| M2 | 缩放与平移 | 1 d | 2.5 d |
| M3 | 目录导航与全部入口 | 0.5~1 d | 3.5 d |
| M4 | 异步加载与大图保护 | 1 d | 4.5 d |
| M5 | 打磨、测试、打包 | 1 d | 5.5 d |
| 缓冲 | 调试、环境问题、评审修改 | 1 d | 6.5 d |

> 原则：每个里程碑结束都产出“可运行、可演示”的状态，不做大爆炸式集成。

### M0 脚手架（0.5 d）

- [x] `cargo init --name image-view`，初始化 git 与 `.gitignore`（`/target`）。
- [x] 配置 `Cargo.toml` 依赖与 release profile。
- [x] `main.rs`：`NativeOptions`（1100×760、最小尺寸、拖放开启）、`run_native`。
- [x] `app.rs`：空 `App` 实现，深色主题，中央显示占位文字（验证中文字体）。
- [x] `assets/icon.png` 占位 + 窗口图标。
- [x] 跑通 `cargo run`，确认 wgpu 正常工作。

**验收**：空窗口可缩放/关闭；中文占位不显示方块；`cargo clippy` 无警告。

### M1 打开与显示（1 d）

- [x] `loader.rs`：先实现同步 `decode()` + 阈值常量 + 降采样函数（接口按 `request/poll` 设计，M4 换线程不换签名）。
- [x] `document.rs`：`LoadedImage`，`ColorImage::from_rgba_unmultiplied` + `ctx.load_texture`。
- [x] 入口：`Ctrl+O`（rfd）、CLI 参数、拖放（`ctx.input(|i| i.raw.dropped_files)`）。
- [x] `viewer.rs`：仅 Fit 显示 + 深色背景 + 无图占位。
- [x] `ui.rs`：底部状态栏（文件名/尺寸/缩放）。
- [x] 错误横幅与日志。

**验收**：打开 24MP JPEG 显示正确；损坏文件不崩溃；中文/空格路径正常。

### M2 缩放与平移（1 d）

- [x] `ViewState` 数学 + 单元测试（锚点、钳制、Fit、边界 DPI）。
- [x] 滚轮/捏合缩放；光标锚点。
- [x] 拖拽平移；边界钳制；拖拽进入 Manual。
- [x] 双击、`F`、`0`、`1`、工具栏按钮切换模式。
- [x] 状态栏百分比（相对纹理）；窗口 resize 行为。
- [x] 100% 物理像素 1:1 在 125%/150%/200% DPI 下验证（125% 实机验证；150%/200% 由 `scale_respects_dpi` 单元测试覆盖，未切换系统缩放实测）。

**验收**：锚点不漂移；平移不出界；模式切换无跳变。

### M3 目录导航与全部入口（0.5~1 d）

- [x] `folder.rs`：扫描、过滤、`natord` 排序、定位索引。
- [x] 上一张/下一张（按钮 + `←/→`），首尾禁用。
- [x] 拖放多文件/目录策略；CLI 目录参数。
- [x] 窗口标题同步（去重）。
- [x] 状态栏序号 `3/128`。

**验收**：自然排序正确；快速翻页无残留；标题同步。

### M4 异步加载与大图保护（1 d）

- [x] `loader.rs` 改为常驻 worker + `mpsc` + `generation`。
- [x] latest-wins：worker 解码前排空请求队列。
- [x] 主线程 `poll_loader`：丢弃过期结果、上传纹理、清空 pending。
- [x] 加载 spinner；错误与过期结果处理。
- [x] 上限：`MAX_SOURCE_PIXELS` / `MAX_TEXTURE_SIDE`（读 `ctx.input(|i| i.max_texture_side)`）/ `MAX_TEXTURE_PIXELS`。
- [x] `image::Limits` 限制解压内存；先读尺寸再解码。
- [x] 状态栏“已降采样”提示。

**验收**：100MP 图片打开时 UI 可交互；连按 `→` 只显示最后一张；内存峰值符合预算。

### M5 打磨与验收（1 d）

- [x] 帮助窗口/快捷键提示；按钮 tooltip。
- [x] 透明棋盘格的视觉打磨（缩放时的格子尺寸、与大图边界的对齐）。
- [x] 加载失败后的重试/关闭横幅。
- [x] 完整的 §9.2 手工测试矩阵（环境受限项见 §9.5）。
- [x] release 构建，记录 exe 体积与启动时间。
- [x] 写 README（运行方式、快捷键、支持格式、已知限制）。
- [x] 代码整理：模块可见性、注释、`#[allow]` 清理。

**验收**：§1.4 DoD 全部通过。

### M6 v1.1 增量：EXIF 方向与旋转（0.5 d，已完成）

- [x] EXIF Orientation 自动校正（JPEG/TIFF/WebP，走 `image` 内置解码器接口）。
- [x] `R` / `Shift+R` 几何旋转 90°，不重传纹理；Fit/缩放/平移按旋转后尺寸计算。
- [x] 帮助窗口与 README 快捷键、已知限制更新。
- [x] 四色象限夹具 + 像素采样验证方向；EXIF Orientation 6 / 3 夹具。

**验收**：§9.6 全部通过。

### M7 v1.1 增量：幻灯片与预加载缓存（0.5 d，已完成）

- [x] 幻灯片：3s 间隔，`Space` / 工具栏切换；手动翻页重置计时；最后一张自动停止。
- [x] 单解码线程优先级队列：`Open` 优先于 `Preload`；新 `Open` 取消排队的预加载。
- [x] 相邻图片预加载（前后各一张），原图 > 30 MP 时跳过。
- [x] LRU 纹理缓存：最多 3 张 / 72M 像素；命中直接显示并作废在途解码结果。
- [x] 工具栏选中态、状态栏“幻灯片中”、帮助窗口 `Space` 行。
- [x] 单元测试与 GUI 计时/内存实测（§9.7）。

**验收**：§9.7 全部通过。

---

## 9. 测试计划

### 9.1 单元测试（`#[cfg(test)]`）

| 模块 | 用例 | 期望 |
| --- | --- | --- |
| `viewer` | `fit_zoom(1000×800 view, 4000×3000 img, ppp=1)` | 0.25 |
| `viewer` | Fit 小图（200×100）不放大 | `zoom == 1.0`，居中 |
| `viewer` | `zoom_at` 锚点：缩放前后光标下的纹理点相同 | 误差 < 1e-3 |
| `viewer` | 缩放范围钳制 | `[0.01, 32.0]` |
| `viewer` | 平移钳制：图像小于画布 | `pan == 0` |
| `viewer` | 平移钳制：图像大于画布 | 边界不露背景 |
| `viewer` | ppp=1.5 时 `scale = zoom / 1.5` | 一致 |
| `loader` | `target_size(10000×5000, max_side=8192)` | `8192×4096` |
| `loader` | `target_size` 受 `max_pixels` 限制 | 总像素 ≤ 上限 |
| `loader` | 降采样尺寸不改变宽高比 | 比例误差 < 1 px |
| `folder` | `["img10.jpg", "img2.jpg", "img1.jpg"]` | `img1, img2, img10` |
| `folder` | 扩展名大小写混排 | 全部识别 |
| `folder` | 过滤目录与非图片文件 | 不包含 |
| `viewer` | 旋转 90°/270° 后显示尺寸 | 宽高交换；180° 不变 |
| `viewer` | 旋转后的 Fit 缩放 | 4000×3000 旋转 90° 在 1000×800 画布中为 0.2 |
| `viewer` | `rotate(-1)` 取模与居中 | `rotation == 3`，`pan == 0` |
| `viewer` | 旋转 UV 表（90°） | 屏幕左上取原图左下，即顺时针 |
| `app` | `R` / `Shift+R` 的旋转步数 | +1 / -1；其他按键为 0 |
| `loader` | EXIF Orientation=6 的 JPEG | 显示尺寸 20×40，上红下蓝 |
| `app` | 标题格式化为 `name — 6000×4000 (67%)` | 一致 |
| `cache` | 插入第 4 张（上限 3） | 淘汰最旧，保留最后 3 条 |
| `cache` | 命中后再插入新图 | 命中项刷新为最近使用，不被淘汰 |
| `cache` | 当前图片是最旧条目 | 保留当前图片，淘汰其他最旧项 |
| `cache` | 总像素超过预算 | 从最旧开始淘汰直到满足预算 |
| `cache` | 查询未缓存路径 | `get` 返回 `None`，`contains` 为 `false` |
| `loader` | 队列含多个 `Open` | 只保留最后一个（latest-wins） |
| `loader` | 新 `Open` 遇到排队 `Preload` | 清空预加载，只留 `Open` |
| `loader` | 多个 `Preload` | 按路径去重并保持顺序 |
| `loader` | 排队 `Open` + 新 `Preload` | `Open` 不被替换 |
| `loader` | 超过预加载阈值的图片 | `PreloadSkipped`；同一文件 `Open` 成功 |
| `app` | 缓存命中时 `request_load` | 不置 `pending`，代次推进 |
| `app` | 最后一张启动 / 触发幻灯片 | 不启动；运行中自动停止 |
| `app` | 幻灯片运行中手动翻页 | 计时重置，相邻图片命中缓存 |

### 9.2 手工测试矩阵

| 类别 | 用例 | 关注点 |
| --- | --- | --- |
| 格式 | PNG（透明/索引色/16-bit）、JPEG（灰度/CMYK）、GIF、WebP、BMP、TIFF、ICO | 不崩溃、颜色可接受 |
| 尺寸 | 1×1、100×100、1920×1080、6000×4000、20000×10000 | 缩放边界、内存 |
| 超大 | 100MP JPEG、300MP 拒绝、500MB 限制 | 提示清晰、不死机 |
| 路径 | 中文、空格、`#`、emoji、长路径、UNC `\\server\share` | 打开/翻页正常 |
| 文件 | 0 字节、截断 JPEG、伪装扩展名、无权限 | 错误横幅、不 panic |
| 目录 | 10k 文件、混合格式、隐藏文件、无图片 | 扫描速度、空状态 |
| 显示 | 125%/150%/200% DPI、多显示器拖动、窗口最小尺寸 | 100% 语义、Fit 重算 |
| 交互 | 快速连按 `→`、滚轮缩放边界、拖拽出界、双击 | 只显示最后一张、无漂移 |
| 加载 | 加载中再打开、加载中关闭窗口 | 无 panic、worker 退出 |
| 平台 | Windows 10/11；macOS/Linux 编译 | 仅 Windows 深度测试 |

### 9.3 性能测试

- 启动：`Instant::now()` 到首帧；用 release 连续测 5 次取中位数。
- 打开：从触发到纹理上传完成，日志记录 `decode` / `resize` / `upload` 三段耗时。
- 交互：用 egui 的帧时间或 Windows 任务管理器观察；目标 60 fps。
- 内存：任务管理器“专用工作集”；记录 24MP 与 40MP 峰值。
- 空闲：不操作 30s，CPU 应接近 0%。

### 9.4 质量门禁

```text
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo build --release
```

可选 CI：GitHub Actions `windows-latest`，缓存 `~/.cargo` 与 `target`，跑上述四条。

### 9.5 实施与验证记录（2026-11-10）

**环境**：Windows 11 Pro 10.0.26300 · i5-11400 · 32 GB · NVIDIA GT 720（wgpu/Vulkan）· 单显示器 125% 缩放 · rustc 1.99.1。

**质量门禁**：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`（17 个单元测试）、`cargo build --release` 全部通过；release exe 约 16.8 MB。

**功能矩阵**（release 构建，启动后核对窗口标题/截图）：

| 类别 | 结果 |
| --- | --- |
| 格式 | PNG（透明/16-bit）、JPEG（彩色/灰度）、GIF、WebP、BMP、TIFF、ICO、QOI、TGA、PNM、HDR、EXR 均正常打开；CMYK JPEG 缺少夹具，未覆盖 |
| 尺寸 | 1×1、100×100、320×240、1920×1080、6000×4000、10000×10000 正常；20000×15000（300MP，仅文件头）被安全上限拒绝并显示横幅 |
| 异常文件 | 0 字节 PNG、截断 JPEG、伪装 `.png` 文本、拒绝读取（ACL）均显示错误横幅且不崩溃；真实 PNG 改名为 `.txt` 可正常打开 |
| 路径 | 中文、空格、`#`、329 字符长路径、UNC（`\\localhost\C$\...` 文件与目录）均正常 |
| 目录 | 10001 个文件、仅含隐藏文件、空目录均正常；隐藏文件会被扫描到，空目录提示无图片 |
| 交互 | 快速连按 `→` 只显示最后一张；首张或后续图片解码中按 `→` 均会切换目标，过期结果被丢弃（从未显示中间图）；缩放/平移边界由单元测试与截图确认 |
| DPI | 125% 下 Fit 与 100% 语义正确（100% 已逐像素核对） |
| 加载 | 加载中切换目标正常；加载中关闭窗口未单独验证 |

**性能实测**：

| 指标 | 预算 | 实测 | 结论 |
| --- | --- | --- | --- |
| 冷启动到窗口可见 | ≤ 600 ms | 5 次中位数 337 ms（322~470） | 达标 |
| 12MP JPEG 打开（进程启动到标题） | ≤ 350 ms | 413 ms，扣除约 340 ms 空启动后约 70 ms | 达标 |
| 24MP JPEG 打开（进程启动到标题） | ≤ 600 ms | 474~495 ms（含启动） | 达标 |
| 100MP JPEG 打开 | ≤ 4 s | 3.5 / 4.4 / 3.5 s | 中位数达标，偶发 4.4 s |
| 缩放/平移帧率（24MP，100%，连续重绘） | 60 fps | p50 16.68 ms（60.0 fps），p95 18.06 ms | 达标 |
| 空闲 CPU | ≈ 0% | 10 秒平均 0.00% | 达标 |
| 24MP 专用工作集 | ≤ 250 MB | 259.9 MB（空界面基线 130.8 MB） | 略超 10 MB |
| 40MP 显示上限（100MP 降采样后） | ≤ 450 MB | 284.2 MB | 达标 |
| 目录扫描 | 2 万文件 ≤ 100 ms | 10001 个文件 8.4 ms（debug 构建） | 达标（见下方优化） |

扫描优化：原先逐项 `Path::is_file()` 会为每个目录项额外查询元数据，10001 个文件耗时 278 ms；改用 `DirEntry::file_type()` 复用目录枚举属性后降至 8.4 ms，2 万文件约 17 ms。

**未验证项（受环境限制）**：

- 150% / 200% 系统缩放与跨显示器拖动：仅单显示器 125%，缩放数学由 `scale_respects_dpi` 覆盖。
- macOS / Linux 编译：本机只安装了 `x86_64-pc-windows-msvc` 目标。
- 真实网络 UNC 共享、管理员权限下的拖放（UIPI）、加载过程中关闭窗口。
- 60 fps 在 release 下未直接插桩；上表为 debug（opt-level 1）连续重绘结果，release 不应更差。

**与设计文档的偏差**：

- 棋盘格格子为 24 pt（设计稿写 8 px），与当前窗口和图像缩放比例配合更自然。
- 错误横幅按钮为“关闭”/“重试”，帮助按钮为“帮助”，未使用 `×`/`?` 符号。
- 未单独定义 `LoadError::UnsupportedFormat`；无法猜测格式时统一归入 `Decode` 错误文案。
- `F11` 全屏已实现（原计划列为 P1 的可选项）；无边框沉浸模式仍未做。

### 9.6 v1.1 验证记录：EXIF 方向与 90° 旋转（2026-11-10）

**质量门禁**：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`（24 个单元测试）、`cargo build --release` 全部通过。

**实现要点**：

- EXIF：`into_decoder()` → `orientation()` → `DynamicImage::from_decoder` → `apply_orientation`，然后才计算尺寸、降采样。
- 旋转：`ViewState.rotation`（0~3）只影响显示；`egui::Mesh` 旋转 UV，纹理不重传；换图重置。

**验证方法**：生成 320×240 四色象限图（左上红 / 右上绿 / 左下蓝 / 右下黄），用 `PrintWindow` 截图后逐象限采样平均色，避免肉眼判断方向出错。

| 操作 | 期望（左上/右上/左下/右下） | 实测 |
| --- | --- | --- |
| 原图 | 红 / 绿 / 蓝 / 黄 | 一致 |
| `R` ×1 | 蓝 / 红 / 黄 / 绿 | 一致 |
| `R` ×2 | 黄 / 蓝 / 绿 / 红 | 一致 |
| `R` ×3 | 绿 / 黄 / 红 / 蓝 | 一致 |
| `R` ×4 | 回到原图 | 一致 |
| `Shift+R` | 绿 / 黄 / 红 / 蓝（逆时针） | 一致（单元测试覆盖） |
| EXIF Orientation=6 的 320×240 JPEG | 显示为 240×320，内容顺时针 90° | 一致 |
| EXIF Orientation=3 的 JPEG | 内容旋转 180° | 一致 |
| 1920×1080 在 Fit 下旋转 | 57% → 37%，竖图完整可见、无裁切 | 一致 |

**发现与修复**：

- `consume_key` 使用逻辑修饰键匹配，未按下的 Shift 不会阻止 `Modifiers::NONE` 命中。最初把 `rotate(1)` 绑定到 `Shift+R` 分支，导致 `R` 实际逆时针；改为先匹配 `Shift+R` 并绑定 `rotate(-1)` 后方向正确。该问题只有像素级采样才能稳定发现，肉眼在渐变图上容易误判。
- GUI 脚本用 `PostMessage` 发送按键不会带 Shift，而 `keybd_event` 受前台窗口抢占影响偶发失败；因此把快捷键解析抽成 `consume_rotation()`，用 `egui::Context::run_pass` 注入按键事件做确定性单元测试（`R`→+1、`Shift+R`→-1、其他键→0）。
- 标题与状态栏在画布之前计算，加载或旋转后可能短暂显示上一帧的百分比。现在 `logic()` 中先用上一帧画布尺寸重算 Fit，并在加载完成后补一帧重绘。
- 截图中的“底部半透明条带”是测试图 `% 256` 渐变的自然回绕，不是渲染缺陷（像素值在边界处连续回绕）。

**未覆盖**：

- 仅验证 125% DPI；150% / 200% 与跨显示器未复测（缩放数学由单元测试覆盖）。
- EXIF 方向只在 JPEG 上实测；TIFF/WebP 走同一接口，未单独取夹具。

### 9.7 v1.1 验证记录：幻灯片与预加载缓存（2026-11-10）

**质量门禁**：`cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`（37 个单元测试）、`cargo build --release` 全部通过；release exe 约 16.8 MB。

**GUI 脚本**：生成 4 张 320×240 纯色 PNG（红/绿/蓝/黄），以目录启动，用 `PostMessage` 发送 `Space` / `←` / `→`，按窗口标题判断当前图片并截图。

| 检查项 | 期望 | 实测 |
| --- | --- | --- |
| 启动 | 显示 s1 | 一致 |
| `Space` | 约 3s 后前进到 s2，工具栏与状态栏显示运行中 | 一致 |
| 再按 `Space` | 停止；3.8s 后仍停在 s2 | 一致 |
| 再次 `Space` | 3s 后 s3，再 3s 后 s4 | 一致 |
| 最后一张 | 到达 s4 后自动停止，状态栏不再显示“幻灯片中” | 一致（截图） |
| 末张按 `→` | 无操作 | 一致 |
| `←` / `→` 回翻 | 立即显示，日志出现 `cache hit` | 一致 |

**预加载/缓存日志**（`RUST_LOG=image_view=debug`）：

```text
loaded ...\s1.png (320×240)
preloaded ...\s2.png (320×240)
cache hit ...\s2.png
preloaded ...\s3.png (320×240)
cache hit ...\s3.png
preloaded ...\s4.png (320×240)
cache hit ...\s4.png
```

**24MP 性能**（4 张 6000×4000 JPEG）：

| 指标 | 实测 |
| --- | --- |
| 首张开图（进程启动到标题） | 660 ms |
| 缓存命中的下一张（标题更新） | 0~1 ms |
| 缓存满（3 张 24MP）工作集 | 305.9 MB |
| 回翻 2 张后工作集 | 305.9 MB（无增长） |

**100MP 邻居**：目录中 `s0-small.png` → `s1-huge.jpg`（10000×10000）→ `s2-small.png`。查看 s0 时日志为 `preload skipped ... (10000×10000)`，不会阻塞翻页；打开 s1 耗时 3.9s（走完整解码），随后正常预加载 s2。

**发现与修复**：

- 缓存命中发生在 `logic()` 中时没有输入事件驱动下一帧，标题会短暂显示重置后的 100%。新增 `needs_repaint` 标记，在换图后补一帧，与异步加载路径一致。
- `pixel_budget_evicts_oldest` 初版把当前图片设为最旧条目，按“当前永不淘汰”规则不会淘汰它；修正测试，使预算淘汰使用非当前条目。

**未覆盖**：

- 3 张 40MP 纹理的像素预算淘汰仅由单元测试覆盖，未做 GUI 实测。
- 150% / 200% DPI、跨显示器与多窗口未复测。

---

## 10. 性能预算

| 指标 | 目标 | 测量方式 | 不达标的处理 |
| --- | --- | --- | --- |
| 冷启动到首帧 | ≤ 600 ms | 日志时间戳 | 延迟创建纹理/字体；检查 wgpu 初始化 |
| 12MP JPEG 打开 | ≤ 350 ms | decode/upload 日志 | 检查解码器特性、关闭不必要的缩放 |
| 24MP JPEG 打开 | ≤ 600 ms | 同上 | 检查解码耗时；必要时降低显示上限或提前降采样 |
| 100MP JPEG 打开 | ≤ 4 s（可交互） | 同上 | 先头部尺寸检查、Triangle 缩放 |
| 缩放/平移帧率 | 60 fps | 帧时间 | 避免每帧重传纹理、减少绘制内容 |
| 空闲 CPU | ≈ 0% | 任务管理器 | 移除无意义的 `request_repaint` |
| 24MP 常驻内存 | ≤ 250 MB | 专用工作集 | 上传后释放 CPU 缓冲 |
| 3 张 24MP 缓存（LRU 满） | ≤ 450 MB | 专用工作集 | 调整缓存条数/像素预算 |
| 缓存命中的翻页 | ≤ 50 ms | 标题更新时间 | 检查缓存键与纹理复用 |
| 40MP 上限内存 | ≤ 450 MB | 同上 | 调整 `MAX_TEXTURE_PIXELS` |
| 2 万文件扫描 | ≤ 100 ms | 日志 | 改为 worker 或缓存 |

> 任何优化都以测量数据为准，不提前引入复杂度。

---

## 11. 风险与应对

| 风险 | 可能性 | 影响 | 应对 |
| --- | --- | --- | --- |
| 大图耗尽内存/显存 | 中 | 高 | 解码前尺寸检查 + `image::Limits` + 纹理上限 40MP/8192 |
| 纹理上传卡顿（160MB 一次上传） | 中 | 中 | worker 解码；必要时 P1 分块上传；UI 显示 spinner |
| 解码慢（100MP） | 高 | 中 | Triangle 缩放、优先显示降采样图；P2 两阶段加载 |
| 颜色与 EXIF 方向不符 | 高 | 低 | v1.1 已加 EXIF 方向（JPEG/TIFF/WebP）；PLAN-V2 P1-M3 加 ICC |
| 不支持 HEIC/AVIF | 中 | 中 | 明确格式列表；PLAN-V2 P1-M4 接入 libheif / avif-native |
| wgpu 驱动兼容性 | 低 | 高 | 保留 `glow` 后端切换；日志记录适配器信息 |
| 高 DPI 下 100% 语义错误 | 中 | 中 | 以 `ppp` 换算，专项测试 125/150/200% |
| `rfd` 阻塞 UI 线程 | 中 | 低 | 原生对话框本身模态，可接受；后续可改异步 |
| 管理员权限下拖放失效（Windows UIPI） | 低 | 低 | README 说明“不要以管理员运行” |
| 中文/emoji 字体缺字 | 低 | 中 | 依赖系统字体回退；测试；需要时内嵌 Noto 子集（PLAN-V2 P1-M7） |
| egui 升级破坏 API | 中 | 中 | 锁定 0.37，升级单独 PR，跑手工矩阵 |
| 动画格式只显示首帧 | 高 | 低 | README 声明；PLAN-V2 P1-M2 实现动画 |
| 缓存 3 张纹理抬高常驻内存 | 中 | 低 | 像素预算 72M，当前图片不淘汰；24MP×3 实测约 306 MB |
| 长路径/UNC 兼容 | 低 | 中 | 全程 `PathBuf`；测试 `\\?\` 与 UNC |

---

## 12. 后续路线

v1.1 已完成。v2 及后续需求已迁移到独立计划，本文不再维护路线图：

| 阶段 | 版本 | 主题 | 状态 | 明细 |
| --- | --- | --- | --- | --- |
| P0 | v1.0 / v1.1 | 查看器基线（本文） | ✅ 已完成 | §3.1 / §3.2 |
| P1 | v2.0 | 格式、色彩、浏览与产品化 | ⬜ 未开始 | [PLAN-P1.md](PLAN-P1.md) |
| P2 | v2.x | 专业能力与架构增强 | ⬜ 未开始 | [PLAN-P2.md](PLAN-P2.md) |
| P3 | v3+ | 长期演进（新增） | ⬜ 未开始 | [PLAN-P3.md](PLAN-P3.md) |

进度、里程碑看板与更新规则见 [ROADMAP.md](ROADMAP.md)。v1.1 遗留的窗口持久化、Mipmap、回收站/复制路径、文件夹位置已并入 P1-M1 / P1-M5。

---

## 13. 待确认决策

以下为本计划采用的默认值，确认后即可开工；任何一条都可调整：

1. **框架**：采用 eframe/egui（推荐），而不是 iced/Slint/Tauri。
2. **100% 语义**：1 图像像素 = 1 屏幕物理像素（推荐，符合看图软件惯例）。
3. **Fit 是否放大**：不放大超过 100%（推荐）；小图保持原始大小并居中。
4. **翻页是否循环**：不循环，首尾禁用（推荐）。
5. **界面语言**：仅中文（依赖系统字体回退）。
6. **EXIF 方向**：v1.1 已实现（见 §9.6）；其他格式按原始像素显示。
7. **目标平台**：Windows 优先，但代码不使用 Windows 专属 API（除子系统属性/图标）。
8. **安装包/文件关联**：本阶段不做，只交付 exe。

---

## 14. 附录

### A. 核心代码骨架

> 以下为结构示意，函数签名以实际实现为准；egui 0.37 API 已核对。

`src/main.rs`

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod document;
mod folder;
mod loader;
mod ui;
mod viewer;

use std::path::PathBuf;

fn main() -> eframe::Result {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("warn,image_view=info"),
    )
    .init();

    let open_path = std::env::args_os().nth(1).map(PathBuf::from);
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("图片查看器")
            .with_inner_size([1100.0, 760.0])
            .with_min_inner_size([480.0, 320.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "image-view",
        options,
        Box::new(move |cc| Ok(Box::new(app::ImageViewerApp::new(cc, open_path)))),
    )
}
```

`src/app.rs`（编排）

```rust
impl eframe::App for ImageViewerApp {
    // eframe 0.37 起 App 拆成 logic（仅逻辑、不可绘制）与 ui（绘制）两部分。
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_dropped_files(ctx);
        self.handle_shortcuts(ctx);
        self.poll_loader(ctx);
        self.sync_title(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.top_bar(ui);
        self.status_bar(ui);

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::from_gray(18)))
            .show(ui, |ui| self.canvas(ui));
    }
}
```

> 注意：egui 0.37 移除了 `TopBottomPanel`/`SidePanel`，统一为 `egui::Panel::top/bottom/left/right`；`Panel::show` 与 `CentralPanel::show` 的第一个参数都是 `&mut Ui`（面板要加到根 `Ui` 上，而不是 `Context`）。

`src/viewer.rs`（变换数学节选）

```rust
pub const MIN_ZOOM: f32 = 0.01;
pub const MAX_ZOOM: f32 = 32.0;

impl ViewState {
    pub fn scale(&self, ppp: f32) -> f32 {
        self.zoom / ppp
    }

    pub fn fit(&mut self, view: Vec2, image: Vec2, ppp: f32) {
        let z = (view.x * ppp / image.x).min(view.y * ppp / image.y);
        self.zoom = z.min(1.0).max(MIN_ZOOM);
        self.pan = Vec2::ZERO;
        self.mode = ViewMode::Fit;
    }

    pub fn zoom_at(&mut self, cursor: Vec2, image: Vec2, view: Vec2, factor: f32, ppp: f32) {
        let s0 = self.zoom / ppp;
        let p = (cursor - self.pan) / s0;
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        let s1 = self.zoom / ppp;
        self.pan = cursor - p * s1;
        self.mode = ViewMode::Manual;
        self.clamp_pan(image, view, ppp);
    }

    pub fn clamp_pan(&mut self, image: Vec2, view: Vec2, ppp: f32) {
        let disp = image * (self.zoom / ppp);
        let limit = ((disp - view) / 2.0).max(Vec2::ZERO);
        self.pan.x = self.pan.x.clamp(-limit.x, limit.x);
        self.pan.y = self.pan.y.clamp(-limit.y, limit.y);
    }
}
```

`src/loader.rs`（worker 节选）

```rust
pub struct Loader {
    tx: Sender<Request>,
    rx: Receiver<Response>,
    next_gen: u64,
}

impl Loader {
    pub fn new(ctx: egui::Context, caps: Caps) -> Self {
        let (tx_req, rx_req) = mpsc::channel::<Request>();
        let (tx_res, rx_res) = mpsc::channel::<Response>();

        std::thread::Builder::new()
            .name("decoder".into())
            .spawn(move || {
                while let Ok(mut req) = rx_req.recv() {
                    // latest-wins：排空排队中的旧请求
                    while let Ok(newer) = rx_req.try_recv() {
                        req = newer;
                    }
                    let result = decode(&req.path, &caps);
                    if tx_res.send(Response { generation: req.generation, result }).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            })
            .expect("spawn decoder");

        Self { tx: tx_req, rx: rx_res, next_gen: 0 }
    }

    pub fn request(&mut self, path: PathBuf) -> u64 {
        self.next_gen += 1;
        let _ = self.tx.send(Request { generation: self.next_gen, path });
        self.next_gen
    }

    pub fn try_recv(&self) -> Option<Response> {
        self.rx.try_recv().ok()
    }
}
```

### B. egui 升级检查清单

升级 egui/eframe 时逐项确认：

- `run_native` 的 `AppCreator` 返回类型（`Result<Box<dyn App>, ...>`）。
- `ViewportBuilder` 的方法（`with_drag_and_drop`、`with_icon`）是否保留。
- `TextureHandle` / `Context::load_texture` / `ColorImage` 构造方式。
- `Painter::image` 与 `TextureOptions`（`mipmap_mode` 为 0.37 新增）。
- `InputState` 字段：`smooth_scroll_delta`、`zoom_delta()`、`max_texture_side`、`raw.dropped_files`。
- `Response` 的 `dragged/drag_delta/double_clicked` 行为。
- `Frame::NONE` 与面板 API。

### C. 参考资料

- egui 文档：<https://docs.rs/egui/0.37.0/egui/>
- eframe 文档：<https://docs.rs/eframe/0.37.0/eframe/>
- image 文档：<https://docs.rs/image/0.25.10/image/>
- rfd 文档：<https://docs.rs/rfd/0.18.0/rfd/>
- egui 示例仓库：<https://github.com/emilk/egui/tree/master/examples>
