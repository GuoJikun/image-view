# ImageViewer 架构设计：eframe + egui + Rust

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

# 架构设计 A：eframe + egui + Rust

## 1. 方案定位

适合希望完全使用 Rust 开发
UI、快速实现跨平台桌面应用，并且看重图片画布交互的方案。egui 是
immediate-mode GUI；eframe 提供应用运行与窗口集成。

优点： - UI、事件和业务都可以在 Rust 类型系统内组织。 - egui
的绘制模型适合自定义图片画布、缩放、平移和叠加层。 - eframe 可以覆盖
Windows、macOS、Linux 等桌面平台。 - 不需要 WebView，核心和 GUI 之间无需
JSON IPC。

需要权衡： - immediate-mode 的状态组织必须有纪律，不能把所有业务逻辑堆在
`update()`。 - 大型管理界面、复杂表单和成熟组件生态需要更多自建。 -
图片像素上传为 GPU texture 的生命周期、上传频率和内存预算需要专门设计。

## 2. 依赖建议

-   `eframe`：应用入口、窗口与事件循环。
-   `egui`：UI、输入事件、绘制。
-   `egui_extras`：可选的表格、图片等辅助能力；不要让其承担核心解码。
-   `image`：基础图片解码，按格式需要评估其他解码器。
-   `fast_image_resize`：缩放。
-   `rayon`：CPU 密集型工作。
-   `tokio`：只有在需要异步 IO/任务协调时引入。
-   `rusqlite`：SQLite。
-   `tracing`：日志。
-   `rfd`：跨平台文件对话框。
-   `directories`：平台标准目录。
-   `notify`：文件变化监听。

版本与功能以项目实际锁定版本为准；先验证所选 eframe/egui 版本的
renderer、平台后端和打包要求。

## 3. 应用结构

``` text
apps/eframe-app/
├── Cargo.toml
└── src/
    ├── main.rs
    ├── app.rs                 # eframe::App 实现，仅做编排
    ├── app_state.rs           # UI 临时状态
    ├── screens/
    │   ├── viewer.rs
    │   ├── library.rs
    │   ├── settings.rs
    │   └── batch.rs
    ├── widgets/
    │   ├── image_canvas.rs
    │   ├── thumbnail_strip.rs
    │   ├── toolbar.rs
    │   └── metadata_panel.rs
    ├── adapters/
    │   ├── core_events.rs
    │   ├── texture_registry.rs
    │   └── dialogs.rs
    └── theme/
```

`app.rs` 只负责每帧调用各 Screen、处理全局菜单和请求重绘；不要在
`update()` 中执行同步图片解码或目录扫描。

## 4. UI 与 Core 的边界

``` text
egui Input
   ↓
UI Action
   ↓
Application Use Case
   ↓
Core / Ports
   ↓
Result + Domain Event
   ↓
egui View State
   ↓
Painter / TextureHandle
```

定义 GUI 专属动作，例如：

``` rust
pub enum ViewerAction {
    OpenPath(PathBuf),
    Next,
    Previous,
    SetZoom(f32),
    FitToWindow,
    RotateLeft,
    ToggleInfoPanel,
}
```

这些动作属于 eframe Adapter，不放入 Domain。Adapter 将动作转换为
Application 用例调用。

## 5. 图片画布与纹理管理

egui 绘制图片通常需要把像素数据上传为 `TextureHandle`。建议建立独立
`TextureRegistry`：

-   以 `ImageId + revision + target_size` 为资源键。
-   只在资源版本变化时上传纹理。
-   视口变化时请求合适分辨率的预览，而不是每帧重新缩放和上传。
-   限制 GPU 纹理缓存大小，并在淘汰时释放纹理句柄。
-   处理纹理尺寸上限、设备丢失和重建。
-   画布坐标与图片坐标使用明确的变换矩阵/函数。
-   鼠标位置缩放时保持鼠标指向的图片位置不变。

建议首期使用 egui 的普通纹理绘制完成单图查看；不要立即实现自定义 wgpu
render pass。只有在分块超大图、色彩管理或高级 GPU
特效成为明确需求时，再考虑自定义渲染器。

## 6. 状态管理

egui 的 UI 状态由 `EframeAppState` 持有，但业务数据仍由 Application/Core
管理。

``` rust
pub struct EframeAppState {
    pub active_screen: Screen,
    pub viewer: ViewerUiState,
    pub show_metadata: bool,
    pub show_thumbnails: bool,
    pub texture_registry: TextureRegistry,
    pub pending_actions: Vec<ViewerAction>,
}
```

不要将 `ImageData { pixels: Vec<u8> }` 放进 UI 状态；只保存
ImageId、PreviewDescriptor、TextureHandle 和视图参数。

## 7. 后台任务与重绘

后台任务通过 channel 或共享事件队列向 UI 发送轻量结果：

-   `ImageOpened`
-   `PreviewReady`
-   `MetadataReady`
-   `DirectoryListed`
-   `TaskProgress`
-   `TaskFailed`

收到事件后更新 UI 状态并请求 repaint。不要让后台线程直接操作 egui
Context 以外的 UI 状态；跨线程通信必须明确同步策略。

对快速切换图片使用 generation id，旧请求即使完成也不得覆盖新选择。

## 8. 系统集成

eframe 不自动解决所有文件关联与单实例需求。复用统一 `iv-platform`：

-   启动参数解析：打开文件路径。
-   单实例：平台 adapter 负责 IPC，将新路径发送给已运行实例。
-   文件关联：安装器/平台包负责注册。
-   macOS 文件打开事件和 Linux 桌面启动参数需分别验证。
-   文件对话框使用统一抽象，底层可使用 `rfd`。

## 9. 构建与发布

每个平台分别构建与打包。CI 至少覆盖： - Windows x86_64 -
macOS（按目标架构分别构建或提供 universal 方案） - Linux x86_64

验证 GPU 后端、系统依赖、图标、文件关联、桌面入口和升级行为。Linux
发布格式可根据目标发行版选择 AppImage、deb、rpm
等，不要假设一个包覆盖所有桌面环境。

## 10. 本方案的 GUI 替换成本

替换为 Iced 或 Tauri 时需要替换： - `apps/eframe-app` - egui
Screen/Widget - `TextureRegistry` - egui 输入事件到 Application Action
的映射 - eframe 窗口与菜单适配

无需替换： - `iv-domain` - `iv-application` -
解码、处理、渲染计划、缓存、元数据、文件、数据库、任务和平台抽象（平台
adapter 若与 GUI 生命周期耦合的部分除外）。

## 11. 风险与 POC 验收

先做一个 POC，验证： 1. 1 亿像素级图片打开时 UI 不冻结。 2.
连续快速切换图片不会显示过期结果。 3. TextureHandle 不会无限增长。 4. 高
DPI、不同缩放比例和窗口尺寸下画布坐标正确。 5. Windows/macOS/Linux
的文件打开、单实例和安装包可用。 6. 长时间浏览大量图片时内存和 GPU
资源可控。

如果 POC 通过，eframe/egui 是一条纯 Rust、画布交互能力较强的路线。
