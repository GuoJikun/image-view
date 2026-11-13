# ImageViewer 架构设计：Tauri 2 + Vue 3 + Rust

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

# 架构设计 C：Tauri 2 + Vue 3 + Rust

## 1. 方案定位

适合重视复杂 UI、设计一致性、快速迭代和 Vue
生态，同时希望图片处理、文件系统、数据库和任务调度由 Rust 负责的方案。

优点： - Vue 3 + TypeScript 适合复杂管理界面、缩略图布局、面板和主题。 -
Rust Core 可以完全独立于 Tauri。 - Tauri Commands、Events
和自定义协议提供前后端边界。 - Fluent 2 风格可以用
CSS/组件系统灵活实现。

需要权衡： - 依赖系统 WebView，三个平台的 WebView 版本和行为存在差异。 -
大图像数据不能通过 JSON/Base64 反复传输。 - WebView 的
Canvas/WebGL/WebGPU 能力需要按平台检测。 - Tauri
插件和前端依赖需要分别审计许可证、安全和兼容性。

## 2. 依赖建议

Rust： - `tauri`：桌面壳和 IPC。 - `serde`：DTO 序列化。 - `tokio`：异步
IO 和任务协调。 - `rayon`：CPU 密集型图片处理。 -
`image`：基础图片解码。 - `fast_image_resize`：缩放。 -
`rusqlite`：SQLite。 - `tracing`：日志。 - `directories`：标准目录。 -
`notify`：文件监听。

前端： - Vue 3 + TypeScript - Vite - Pinia - VueUse（按需） - CSS
Variables / Fluent 2 自建组件 - Canvas 2D 起步；需要时评估 WebGL2/WebGPU

依赖应按实际需要引入，避免为了使用某个生态而安装大量不必要插件。

## 3. 应用结构

``` text
apps/tauri-app/
├── src-tauri/
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/
│   └── src/
│       ├── main.rs
│       ├── lib.rs
│       ├── setup.rs
│       ├── state.rs
│       ├── commands/
│       │   ├── image.rs
│       │   ├── library.rs
│       │   ├── file.rs
│       │   └── task.rs
│       ├── events/
│       ├── protocol/
│       └── adapters/
└── frontend/
    ├── src/
    │   ├── main.ts
    │   ├── App.vue
    │   ├── layouts/
    │   ├── views/
    │   ├── components/
    │   │   ├── ImageCanvas/
    │   │   ├── ThumbnailStrip/
    │   │   └── MetadataPanel/
    │   ├── stores/
    │   ├── services/
    │   ├── composables/
    │   ├── types/
    │   └── styles/
    └── package.json
```

`src-tauri` 只做桌面入口、状态装配、Command/Event/Protocol
Adapter，不应成为所有业务逻辑的存放位置。它通过依赖 workspace crates
使用统一 Application/Core。

## 4. IPC 边界

将通信划分为三类：

### Commands：短请求/响应

例如： - `open_image` - `list_directory` - `get_metadata` -
`rotate_image` - `scan_directory` - `search_images` - `add_favorite` -
`cancel_task`

Command 只负责参数校验、调用 Application 用例和 DTO 转换。不要实现一个
`execute_action(action, params: Value)` 的万能接口。

### Events：异步状态

例如： - `image:preview-ready` - `image:metadata-ready` -
`directory:scan-progress` - `task:progress` - `task:completed` -
`task:failed`

事件应携带 request_id/task_id
和资源版本号。前端只接受当前有效请求的结果。事件 payload
保持轻量，不携带大块像素数据。

### Custom Protocol：图片资源

通过受控的自定义资源协议提供预览，例如：

``` text
iviewer://preview/{image_id}/{revision}
iviewer://thumbnail/{image_id}/{size}/{revision}
```

协议 handler 通过授权的资源 ID 查找 Rust
缓存，不允许把任意文件路径直接拼进 URL
并读取。应校验资源访问范围、处理缓存头、取消请求、资源失效和并发读取。

协议返回可被 WebView 解码的图像格式，或经评估后的二进制资源。不要用
Base64 放进 JSON。对于频繁更新的大图，需比较 PNG/WebP
编码开销与原始像素传输成本。

## 5. 前端图片画布

第一版建议 Canvas 2D 或 CSS Transform 实现： -
缩放、平移、旋转、翻转。 - 鼠标位置中心缩放。 -
适应窗口和实际像素大小。 - 高 DPI 适配。 - 画布尺寸与资源尺寸分离。 -
只在图像资源或视图参数变化时重绘。

大图可采用分辨率金字塔/瓦片策略，但解码计划和瓦片缓存规则留在
Core；前端只请求当前视口需要的资源。WebGL2/WebGPU 作为可选
renderer，不应成为基础功能的硬性前提。

## 6. 前端状态

Pinia 保存： - 当前 ImageId、目录图片列表和索引。 - 图片元数据 DTO。 -
缩放、旋转、平移、适应模式。 - 面板显隐、选中状态。 -
任务进度和错误摘要。

Pinia 不保存： - 完整原始像素数组。 - 大型 Base64 字符串。 - Rust
内部缓存对象。 - 数据库连接或文件句柄。

前端 service 负责调用 Commands、订阅 Events 和管理
request_id；组件不应直接散落调用 Tauri API。

## 7. Rust 后端装配

`AppState`
只持有应用服务或服务容器，不要把所有数据库、缓存和解码器状态都暴露给
Command。

推荐启动流程： 1. 初始化 tracing。 2. 初始化配置目录和缓存目录。 3. 打开
SQLite 并执行迁移。 4. 创建任务调度器、缓存和解码服务。 5. 装配
Application 用例。 6. 注册 Commands、Events、Custom Protocol。 7.
初始化文件打开事件、单实例和平台集成。

## 8. 系统集成

复用 `iv-platform`： - Windows 文件关联与启动参数。 - macOS document
types / 文件打开事件。 - Linux desktop entry / MIME。 - 单实例 IPC。 -
剪贴板和文件管理器定位。 - 文件监听。

Tauri 插件可以作为 Adapter 使用，但 Application/Core 不得依赖具体插件
crate。

## 9. 安全与能力配置

-   使用最小化 Tauri capabilities。
-   只开放前端实际需要的 Command 和事件。
-   自定义协议只能读取应用授权的资源。
-   路径校验和文件操作权限在 Rust 侧执行。
-   不信任前端传来的路径、ImageId、任务 ID。
-   文件删除、覆盖、批量修改等操作要有明确确认和错误恢复策略。
-   对第三方插件和 npm/Rust 依赖执行许可证及安全审计。

## 10. 构建与发布

分别验证 Windows、macOS、Linux 的系统 WebView： - Windows WebView2
Runtime 的安装和版本策略。 - macOS WKWebView 行为。 - Linux WebKitGTK
运行时依赖及目标发行版支持。 - 文件关联、单实例和文件打开事件。 -
应用图标、安装包、升级和卸载。 - Canvas 在高 DPI 和不同 WebView
下的表现。

## 11. 本方案的 GUI 替换成本

替换为 eframe 或 Iced 时需要替换： - `apps/tauri-app` - Vue
页面、组件、Pinia Store 和 CSS - Commands/Events/Custom Protocol
Adapter - WebView 图片资源加载与 Canvas 实现 - Tauri 窗口和系统菜单适配

无需替换统一 Domain、Application
用例、图片解码/处理/缓存/元数据、文件管理、数据库、任务核心和平台端口。

## 12. 风险与 POC 验收

POC 必须验证： 1. Windows、macOS、Linux 的 WebView 运行时与打包策略。 2.
预览资源协议在大量图片切换时的性能和缓存行为。 3. 大图不经 Base64/JSON
传输。 4. 快速切换时旧请求不会覆盖新图。 5. WebView Canvas 在高 DPI
下缩放、旋转和拖动正确。 6. 文件打开事件、单实例和多次打开文件行为正确。
7. capabilities 最小化后所有业务流程仍可正常工作。
