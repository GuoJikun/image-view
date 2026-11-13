# ImageViewer 架构设计：Iced + Rust

# 统一架构基线（适用于三套 GUI 方案）

## 1. 项目目标与设计原则

项目定位为跨平台高级图片查看与管理软件，目标平台为 Windows
10/11、macOS、Linux。支持直接打开图片、目录浏览、缩放平移、旋转翻转、缩略图、EXIF、动画图片、收藏标签、搜索、批量处理及后续格式扩展。

必须遵守以下边界：

1.  **Core 不依赖 GUI**：核心 crate 不依赖
    eframe、egui、Iced、Tauri、Vue、WebView 或窗口系统。
2.  **业务用例不依赖
    GUI**：打开图片、切换图片、扫描目录、收藏、转换等用例由 Application
    层实现。
3.  **GUI 只做展示和交互**：GUI
    将用户操作转换为用例请求，将用例状态转换为界面模型。
4.  **平台能力通过端口抽象**：文件关联、单实例、剪贴板、文件监听、系统菜单等由
    Platform Adapter 实现。
5.  **图片数据不进入 UI 全局状态**：Pinia/egui state/Iced state 只保存
    ID、元数据、视图参数和任务状态；像素缓冲区由 Rust Core 持有。
6.  **按需引入复杂能力**：第一阶段不强制实现
    RAW、PSD、云同步、人脸识别或插件系统。
7.  **切换 GUI 的验收标准**：同一组 Core/Application 集成测试必须在三种
    GUI 方案中保持通过。

## 2. 推荐 Workspace

``` text
image-viewer/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── deny.toml
├── crates/
│   ├── iv-domain/             # 领域类型、ID、错误、值对象
│   ├── iv-application/        # 用例、服务编排、端口定义
│   ├── iv-image-decode/       # 格式识别与解码
│   ├── iv-image-process/      # 旋转、翻转、缩放、裁剪
│   ├── iv-image-render/       # 视口、采样尺寸、瓦片/预览策略
│   ├── iv-cache/              # 内存缓存、磁盘缩略图缓存
│   ├── iv-metadata/           # EXIF、ICC、GPS、文件元数据
│   ├── iv-library/            # SQLite、索引、收藏、标签、搜索
│   ├── iv-files/              # 目录遍历、文件操作、排序
│   ├── iv-task/               # 后台任务、取消、进度、并发限制
│   ├── iv-platform/           # 平台端口与跨平台实现
│   ├── iv-infra/              # SQLite、配置、日志等基础设施适配
│   └── iv-gui-contract/       # GUI 无关的 ViewModel/消息 DTO（可选）
├── apps/
│   ├── eframe-app/
│   ├── iced-app/
│   └── tauri-app/
│       ├── src-tauri/
│       └── frontend/
├── tests/
│   ├── fixtures/
│   └── architecture/
└── docs/
```

`iv-gui-contract` 不是必须的。如果 DTO 数量较少，可直接放在
`iv-application`。不要为了抽象而抽象。

## 3. 依赖方向

``` text
GUI App / GUI Adapter
        │
        ▼
iv-application ─────────────── iv-domain
        │                         ▲
        ▼                         │
Ports (traits)                    │
        ▲                         │
        │                         │
iv-infra / iv-platform / image crates
        │
        ├── iv-image-decode
        ├── iv-image-process
        ├── iv-image-render
        ├── iv-cache
        ├── iv-metadata
        ├── iv-library
        ├── iv-files
        └── iv-task
```

更严格地说，Domain 不依赖任何其他项目 crate；Application 依赖
Domain，并定义所需端口；Infrastructure 实现端口。GUI 依赖 Application
暴露的用例接口和 DTO。禁止 Core 反向调用 GUI。

## 4. 核心模块职责

  -------------------------------------------------------------------------------------------------------
  Crate                   主要职责                                                不负责
  ----------------------- ------------------------------------------------------- -----------------------
  `iv-domain`             ImageId、ImageInfo、ImageFormat、Transform、错误类型    文件 IO、GUI

  `iv-application`        OpenImage、Navigate、ScanDirectory、Favorite、Convert   像素绘制
                          等用例                                                  

  `iv-image-decode`       文件签名识别、解码器选择、静态/动画帧读取               窗口显示

  `iv-image-process`      旋转、翻转、裁剪、颜色转换、缩放                        UI 状态

  `iv-image-render`       视口计算、目标分辨率、瓦片计划、渲染资源描述            GUI 控件

  `iv-cache`              LRU 内存缓存、磁盘缩略图缓存、缓存预算                  业务收藏

  `iv-metadata`           EXIF、方向、拍摄时间、GPS、ICC                          数据库 UI

  `iv-library`            SQLite 索引、收藏、标签、相册、FTS 搜索                 实时解码

  `iv-files`              目录枚举、文件属性、排序、文件操作                      图片像素处理

  `iv-task`               任务生命周期、取消、并发控制、进度                      界面进度条

  `iv-platform`           文件关联、单实例、剪贴板、文件监听、系统集成            图片领域规则

  `iv-infra`              配置、日志、SQLite 连接、持久化适配                     UI
  -------------------------------------------------------------------------------------------------------

## 5. 统一领域模型

建议先稳定这些类型，三种 GUI 共享完全相同的定义：

``` rust
pub struct ImageId(pub uuid::Uuid);

pub struct ImageInfo {
    pub id: ImageId,
    pub path: std::path::PathBuf,
    pub format: ImageFormat,
    pub width: u32,
    pub height: u32,
    pub file_size: u64,
    pub modified_at: Option<i64>,
    pub captured_at: Option<i64>,
    pub orientation: Orientation,
}

pub struct ImageView {
    pub scale: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub rotation: Rotation,
    pub flip_x: bool,
    pub flip_y: bool,
    pub fit_mode: FitMode,
}

pub enum FitMode {
    FitWindow,
    ActualSize,
    Custom,
}

pub struct ImageRequest {
    pub image_id: ImageId,
    pub target_width: u32,
    pub target_height: u32,
    pub quality: PreviewQuality,
}

pub struct PreviewDescriptor {
    pub image_id: ImageId,
    pub revision: u64,
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelFormat,
    pub resource: PreviewResource,
}
```

注意：上面是接口示意。实际像素缓冲区应使用受控的内部类型（例如
`Arc<[u8]>` 或专用 Buffer），不要把 `Vec<u8>` 大块复制到 GUI 消息或 JSON
中。`PreviewDescriptor` 只描述如何取得图像数据。

统一错误类型应区分：文件不存在、格式不支持、文件损坏、解码失败、权限不足、资源超限、任务取消、数据库错误。

## 6. Application 层用例与端口

用例建议：

-   `OpenImage`
-   `CloseImage`
-   `NavigateImage`
-   `RequestPreview`
-   `GetImageMetadata`
-   `RotateImage`
-   `FlipImage`
-   `ScanDirectory`
-   `SearchImages`
-   `AddFavorite` / `RemoveFavorite`
-   `AddTag` / `RemoveTag`
-   `DeleteFile` / `RenameFile`
-   `ConvertImages`
-   `CancelTask`

端口示意：

``` rust
#[async_trait::async_trait]
pub trait ImageRepository: Send + Sync {
    async fn get(&self, id: ImageId) -> Result<Option<ImageInfo>, AppError>;
    async fn list_directory(&self, path: &Path) -> Result<Vec<ImageInfo>, AppError>;
}

#[async_trait::async_trait]
pub trait PreviewProvider: Send + Sync {
    async fn request_preview(
        &self,
        request: ImageRequest,
        cancel: CancellationToken,
    ) -> Result<PreviewDescriptor, AppError>;
}

#[async_trait::async_trait]
pub trait TaskReporter: Send + Sync {
    async fn report(&self, event: TaskEvent);
}
```

具体 trait 的 async 方案可选 `async-trait` 或原生 async
trait；需要对象安全时再采用 `async-trait` 或 boxed
future。不要为了异步而把纯 CPU 计算包装成大量 async 函数。

## 7. 图片解码、渲染和传输

建议采用"解码与渲染资源分离"：

1.  文件识别后选择解码器。
2.  读取尺寸、方向、色彩信息。
3.  根据当前视口和缩放比例计算目标预览尺寸。
4.  优先生成适屏预览；超大图按需扩展区域/瓦片解码。
5.  像素数据保存在 Rust 管理的缓存中。
6.  GUI 只拿到资源句柄、尺寸、版本号和元数据。
7.  GUI Adapter 决定如何将资源绘制到画布。

不要默认把所有图片解码成 RGBA8，也不要将 Base64 图片通过 JSON IPC
传输。要明确 stride、像素格式、色彩空间、EXIF 方向和动画帧时序。

首期可先采用单张预览资源；大图分块、GPU 纹理共享、WebGPU/WebGL
等能力作为后续专项设计，不能假设三个 GUI 可以共用同一套 GPU 纹理。

## 8. 缓存与并发

缓存分层：

-   L1：当前图片显示资源。
-   L2：前后相邻图片的预览。
-   L3：缩略图内存缓存。
-   L4：磁盘缩略图/预览缓存。

必须设置内存预算、LRU
淘汰、任务优先级、取消机制和并发上限。缓存键至少考虑
ImageId、文件版本（修改时间/大小或内容指纹）、目标尺寸、变换参数和解码器版本。

Tokio 负责异步 IO、任务协调和文件扫描；Rayon 或专用线程池负责 CPU
密集型解码/缩放。不要在 GUI 主线程执行解码、遍历目录或数据库全量查询。

图片切换采用 request_id /
generation_id：只有最新请求可以更新当前图片，过期任务结果丢弃或进入缓存。

## 9. 数据持久化

建议 SQLite 作为本地数据库，第一版可包含：

-   `images`
-   `directories`
-   `favorites`
-   `tags`
-   `image_tags`
-   `albums`
-   `album_images`
-   `scan_history`
-   `schema_migrations`

图片查看必须不依赖数据库索引：用户直接打开单张图片时，即使尚未建立图片库，也能正常查看和浏览同目录文件。

图片文件是原始数据源；数据库是可重建索引；缓存可删除重建。不要把唯一业务数据放入缓存目录。

## 10. Platform 抽象

`iv-platform` 对上层提供统一接口，平台实现分别处理：

-   文件关联与"打开方式"注册
-   应用启动参数和文件打开事件
-   单实例及 IPC
-   文件系统监听
-   剪贴板
-   在文件管理器中显示文件
-   系统菜单与通知
-   安装包、卸载和升级集成

平台差异必须留在 adapter 内，不得把 Windows Registry、NSWorkspace、DBus
等类型泄漏到 Domain/Application。

## 11. 统一测试策略

三种 GUI 方案必须共享同一组 Core/Application 测试：

-   格式识别和损坏文件处理
-   EXIF 方向
-   旋转、翻转、缩放结果
-   大图内存预算
-   缓存淘汰与失效
-   图片切换的过期任务处理
-   目录增量扫描
-   数据库迁移
-   收藏、标签、搜索
-   任务取消与进度
-   路径含空格、Unicode、非 ASCII 字符
-   跨平台路径与文件关联的集成测试

GUI 测试仅验证输入事件、ViewModel
映射、焦点、键盘快捷键、窗口生命周期和资源展示。

## 12. 分阶段实施

-   P0：Workspace、领域模型、错误体系、CI、许可证审计。
-   P1：单张图片打开、解码、预览、缩放和旋转。
-   P2：目录切换、缩略图、预加载、缓存。
-   P3：EXIF、动画、收藏、标签、SQLite 索引。
-   P4：搜索、批量转换、文件监听、更多格式。
-   P5：大图分块、色彩管理、RAW、性能优化。

每个阶段先实现 Core/Application，再接入 GUI Adapter；避免三个 GUI
分支各自实现业务。

# 架构设计 B：Iced + Rust

## 1. 方案定位

适合希望纯 Rust、声明式
UI、明确状态更新流程，并且准备长期维护复杂桌面应用的项目。

Iced 的核心思路接近 Elm： - State：当前 UI 状态。 -
Message：用户操作或异步结果。 - Update：根据 Message
更新状态并产生任务。 - View：根据状态构建界面。

优点： - 状态流清晰，适合图片库、任务中心、批量操作等复杂业务。 - GUI 与
Core 都使用 Rust 类型，避免跨语言 IPC。 - 可自定义 Widget 和 Canvas。 -
跨平台桌面目标明确。

需要权衡： - 组件生态和成熟度需结合锁定版本评估。 -
复杂图片画布可能需要自定义 Widget/Canvas。 - GPU
资源与图片预览传输仍需要独立管理。 - Iced 版本演进较快，必须锁定版本并做
POC。

## 2. 依赖建议

-   `iced`：UI、Widget、应用状态与事件。
-   `image`：基础图片解码。
-   `fast_image_resize`：缩放。
-   `rayon`：CPU 密集型处理。
-   `tokio`：按需用于异步 IO/任务协调。
-   `rusqlite`：SQLite。
-   `tracing`：日志。
-   `rfd`：文件对话框。
-   `directories`：标准目录。
-   `notify`：文件监听。

Iced 的 renderer、feature flags 和 API
可能随版本变化。开始实现前应锁定一个明确版本，验证 Canvas、图片
Widget、自定义 Widget、窗口事件和各平台 renderer。

## 3. 应用结构

``` text
apps/iced-app/
├── Cargo.toml
└── src/
    ├── main.rs
    ├── app.rs
    ├── message.rs
    ├── state/
    │   ├── app_state.rs
    │   ├── viewer_state.rs
    │   └── library_state.rs
    ├── update/
    │   ├── mod.rs
    │   ├── viewer.rs
    │   ├── library.rs
    │   └── task.rs
    ├── view/
    │   ├── mod.rs
    │   ├── viewer.rs
    │   ├── library.rs
    │   └── settings.rs
    ├── widgets/
    │   ├── image_canvas.rs
    │   ├── thumbnail_strip.rs
    │   └── metadata_panel.rs
    ├── adapters/
    │   ├── core_events.rs
    │   └── image_resource.rs
    └── theme/
```

## 4. 消息与用例映射

Iced Message 只代表 UI 事件或异步结果，不应成为 Domain 的公共事件类型。

``` rust
pub enum Message {
    OpenPath(PathBuf),
    NextImage,
    PreviousImage,
    ZoomChanged(f32),
    PreviewLoaded {
        request_id: u64,
        result: Result<PreviewDescriptor, String>,
    },
    MetadataLoaded {
        image_id: ImageId,
        result: Result<ImageMetadata, String>,
    },
    TaskProgress(TaskProgressDto),
    ToggleMetadataPanel,
}
```

Update 层负责把消息转换为 Application 用例，并把异步结果再映射回
Message。错误需要转换为用户可读状态，但原始错误仍记录在 tracing 日志中。

## 5. State 设计

``` rust
pub struct AppState {
    pub screen: Screen,
    pub viewer: ViewerState,
    pub library: LibraryState,
    pub tasks: Vec<TaskProgressDto>,
}

pub struct ViewerState {
    pub current_image: Option<ImageInfo>,
    pub image_view: ImageView,
    pub preview: Option<PreviewDescriptor>,
    pub request_generation: u64,
    pub show_metadata: bool,
    pub show_thumbnails: bool,
}
```

State 不保存大型像素 Buffer。预览资源由 Core 管理，GUI 侧持有轻量句柄或
renderer 所需资源。

## 6. 图片画布

可先使用 Iced 提供的 Canvas/自定义 Widget 绘制： - 计算图片目标矩形。 -
处理缩放、平移、旋转和翻转。 - 处理鼠标滚轮、拖拽和双击。 -
绘制背景、选中状态和辅助叠加层。

将画布的几何计算放入 GUI 无关的 `iv-image-render`，Iced Canvas
只负责把几何结果映射到 Iced 的绘制 API。这样 egui 和 Tauri
前端也能复用视口计算与变换逻辑。

不要把 Iced 的 `Image` Handle 或 renderer 资源类型泄漏到 Core。

## 7. 异步任务

Iced 的 Task/Subscription 机制负责 UI
生命周期中的异步动作和订阅。Application/Core
仍应有独立任务服务，不能把所有后台工作都写进 Iced Task。

推荐： - Update 触发 Application 用例。 - Core
任务池执行解码、缩放和扫描。 - 轻量结果通过 Iced Task/Subscription 送回
Message。 - 文件监听与任务进度以订阅方式接入。 - UI
销毁或切换时取消不再需要的请求。

需要避免在 Update 中执行同步解码或大规模文件 IO。

## 8. 系统集成

复用 `iv-platform`： - 启动参数和文件打开事件。 - 单实例与 IPC。 -
文件关联。 - 剪贴板。 - 文件监听。 - 文件对话框。 - 平台安装包。

Iced 负责展示系统能力结果，不负责实现平台差异本身。

## 9. 构建与发布

锁定 Iced 版本及 renderer feature。CI 分别验证
Windows、macOS、Linux，尤其验证： - 各平台 renderer 可用性。 - 高 DPI
与窗口缩放。 - Canvas 大图绘制。 - 文件对话框和文件打开事件。 -
打包后字体、图标和资源路径。 - 长时间浏览的内存与 GPU 资源释放。

## 10. 本方案的 GUI 替换成本

替换为 eframe 或 Tauri 时需要替换： - `apps/iced-app` -
Message/Update/View - Iced Canvas 和自定义 Widget - Iced
Task/Subscription 到统一应用事件的适配 - Iced 窗口与菜单适配

无需替换统一 Domain、Application
用例、图片解码/处理/缓存/元数据、文件管理、数据库、任务核心和平台端口。

## 11. 风险与 POC 验收

POC 必须验证： 1. 当前锁定版本的 Canvas/Widget 能否满足图片画布交互。 2.
大图加载不会阻塞 Update/View。 3. 高频 Message 不会造成无界状态增长。 4.
异步任务取消与窗口生命周期正确。 5. 多平台
renderer、文件打开事件、单实例和安装包工作正常。 6. Iced
版本升级不会迫使 Core 改动。

如果希望纯 Rust 且偏好明确的状态更新架构，Iced 值得优先 POC。

## 12. 实施进度（Iced POC）

代码位于 `apps/iced-app`，与 eframe 应用共用全部 Core crate，未修改任何
`crates/*` 的公开 API。锁定版本：`iced 0.14`（`wgpu` + `canvas` +
`image-without-codecs` + `smol`）。

``` bash
cargo run -p image-view-iced -- "D:\photos\a.jpg"
cargo run -p image-view-iced -- "D:\photos"     # 目录取第一张
```

### 12.1 目录与文档草案的差异

| 草案 | 实际 | 原因 |
| --- | --- | --- |
| `adapters/core_events.rs` | `adapters/preview.rs` | Iced 的事件统一走 `iced::event::listen()`，不需要独立的事件总线；真正需要适配的是「后台解码服务」。 |
| `adapters/image_resource.rs` | `adapters/preview.rs`（`PreviewService` / `ThumbnailService`） | 主图复用 `iv-image-decode::Loader`，缩略图用独立 2 线程池，句柄统一为 `iced::widget::image::Handle`。 |
| （未定义） | `adapters/storage.rs` | Iced 没有 egui 的 `Storage`，用一个 RON 键值文件实现 `iv-infra` 的存储端口。 |
| （未定义） | `adapters/errors.rs` | 把 Core 错误类型转成界面文案，避免文案散落在 View。 |
| `theme/`（目录） | `theme.rs`（单文件） | 只有 3 个容器样式，无需拆目录。 |

### 12.2 已实现能力

- **应用骨架**：`main.rs` 负责 CLI、日志与窗口图标，`app.rs` 只做
  boot/update/view/subscription/title/theme 装配。
- **消息映射**：`Message` 只含 UI 事件与异步结果，不进入 Core；键盘、拖放、
  窗口缩放等未被子控件消费的事件由 `iced::event::listen()` 收口。
- **状态**：`AppState` 组合 `iv-application::FolderNav`、
  `ViewerState`（视图变换 + LRU 主图缓存）、`LibraryState`（缩略图缓存 +
  元数据面板）、`Settings` 与 `FileStorage`；GUI 侧不保存像素 Buffer。
- **画布**：`widgets/image_canvas.rs` 实现 `canvas::Program`，几何计算全部来自
  `iv-image-render::ViewState`（滚动缩放锚定光标、拖拽平移、双击切换
  适配/100%、旋转 90° 步进）。
- **缩略图条**：水平虚拟滚动，只为可见区间（外扩 3 张）构建 widget，首尾用
  `Space` 撑出内容宽度；滚动偏移回传后按需排队解码。
- **异步**：主图解码复用 `iv-image-decode::Loader` 的 latest-wins 语义，过期
  结果按 generation 丢弃；`Message::Poll` 每 50 ms 收拢一次，消息量有上界。
- **系统集成**：`rfd` 文件对话框、拖放打开、回收站删除、资源管理器定位、
  复制路径、全屏切换、幻灯片（3 s 自动前进）。
- **持久化**：目录、文件、排序、主题、缩略图条开关写入 `settings.ron`
  （默认 `%APPDATA%\image-view-iced\`，可用
  `IMAGE_VIEW_ICED_CONFIG_DIR` 覆盖）。
- **质量门禁**：`cargo fmt` / `cargo clippy`（零告警）/ `cargo test`
  （14 个单测）通过，启动冒烟测试正常打开图片。

### 12.3 尚未完成

1. 单实例、IPC 与文件关联：需要 `iv-platform` 与安装包层面配合，POC 未接入。
2. 文件监听（`notify`）：当前目录内新增/删除文件后需重新打开目录才刷新。
3. 原生菜单栏与触控板手势：目前只有自绘工具栏与 `⋯` 溢出菜单。
4. 长时浏览的大图 GPU 资源生命周期、macOS/Linux renderer 实机验证。
5. 多语言（i18n）：POC 只提供中文界面，未接入 `iv-domain::Language`。
6. 「跟随系统」主题回落为深色：Iced 0.14 的 `Theme` 没有系统主题探测入口。

