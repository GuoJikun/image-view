# AGENTS.md

面向在本仓库工作的 AI 编码工具。工具无关的全局偏好（中文交流、Angular 提交、此分支永远不会合并到 main 分支）见 `~/AGENTS.md`，
本文件只写 image-view 仓库特有的约定；设计与验收细节以 `docs/` 下的文档为准。

## 项目速览

- 这是 Iced GUI 分支（`gui-iced`）：Rust workspace 承载全部业务逻辑，GUI 是 `app` 下的纯 Rust Iced 0.14 应用。
- 本分支**不合并回 `main`**，只从 `main` 合并 `crates/` 与工作区根 `Cargo.toml`；两者改动要保持兼容。
- `docs/` 下的 eframe / Tauri 架构文档是设计档案，不代表本分支形态；现行架构见 [docs/02-iced-architecture.md](docs/02-iced-architecture.md)。
- 入口文档：[README.md](README.md)（用户视角）、[docs/ROADMAP.md](docs/ROADMAP.md)（进度）、
  `docs/PLAN-P0.md` ~ `docs/PLAN-P3.md`（分阶段设计与验收记录）。

## 质量门禁

改动合入前三条必须全过。不要为了过检查去改断言、放宽阈值或 `--no-verify` 跳钩子：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

本地运行 GUI：`cargo run -p image-view -- <图片或目录路径>`。CI 配置（`*.yml`）由维护者单独维护，改动本分支文档时不要顺手改它。

## 代码分层

- 业务逻辑放 `crates/iv-*`，**不要写进 `app`**；GUI 层只把用户操作映射成 Core 用例、把用例状态映射成界面模型。
  `iv-domain` 领域类型 · `iv-files` 目录扫描排序 · `iv-application` 导航状态 · `iv-metadata` ICC
  · `iv-image-decode` 解码/预加载/缩略图 · `iv-image-render` 缩放平移数学 · `iv-cache` LRU 与像素预算
  · `iv-task` 请求代号 · `iv-platform` 回收站与资源管理器 · `iv-infra` 日志与设置持久化。
- GUI 采用 Elm 结构：`state/` 只存 ID、视图参数与任务状态，**不存像素 Buffer**；像素由 Core 的 `iv-image-decode::Loader` 与 `iv-cache` 持有，GUI 侧只拿 `iced::widget::image::Handle`。
- `message.rs` 的 `Message` 只表示 UI 事件与异步结果，**不进入 Core**；`update/` 负责把消息转成 `iv-application` 用例，并把结果映射回 `Message`。`app.rs` 只做 boot/update/view/subscription/title/theme 装配，不写业务。
- 全局键鼠、拖放、窗口事件统一由 `app.rs` 的 `iced::event::listen()` 收口到 `update/task.rs`；widget 已消费的事件不要重复处理。
- 后台解码结果按 `generation` 判定新旧，过期结果丢弃；结果通过固定周期 `Poll` 收拢进 Update，保证消息量有上界，不要在 Update 里做同步解码或目录遍历。
  相邻图片预加载走 `Purpose::Preload`：它不带 generation 语义，只填 `ViewerState` 的 LRU、不改变当前画面，
  大图由 `Caps::preload_max_source_pixels` 在 Core 侧放弃，别把预加载结果当成打开结果处理。
- 画布几何（缩放锚点、平移钳制、旋转、Fit 计算）全部来自 `iv-image-render::ViewState`，`widgets/image_canvas.rs` 只把结果映射到 Iced 的 `canvas::Program`；不要把 Iced 的 renderer 类型泄漏进 Core。

## GUI 约定

- 新增功能先确认 Core（`crates/iv-*`）是否已有用例；Core 没有时优先在对应 crate 补能力，GUI 只做接线，避免两套 GUI 各自实现业务。
- 界面文案统一放 `app/src/i18n.rs`：中英两份静态表填同一个 `Text` 结构体（漏翻或多写字段编译失败），
  带参数的句子用 `Text` 的方法组句。新增文案先加字段、两份表各填一次，**不要在 `view/` / `widgets/` 里写中文字面量**。
  语言偏好存 `Settings::language`，「跟随系统」由 `iv-platform::system_locale()` 交 `iv-domain::Language::resolve_tag` 解析。
- 偏好只有一处持久化：`app/src/adapters/storage.rs` 的 `FileStorage`（`settings.ron`，原子写）。新增字段走 `iv-infra::Settings`（加 `#[serde(default)]` 保持兼容），在 `state/` 读写并经 `persist` 落盘；不要引入第二份本地存储。
- `settings.ron` 目录可用 `IMAGE_VIEW_ICED_CONFIG_DIR` 覆盖；默认位置按平台取 `%APPDATA%` / `Application Support` / `XDG_CONFIG_HOME`。
- 缩略图磁盘缓存**不是偏好**，走 `iv-cache::FileCache`（`app/src/adapters/preview.rs`），
  目录在平台缓存目录的 `image-view/thumbnails/`，可用 `IMAGE_VIEW_ICED_CACHE_DIR` 覆盖；
  键为「路径摘要 / mtime / 尺寸」，写失败只降级为内存缓存，不要把它并进 `settings.ron`。
- 主题只有 Light / Dark / System 三档，且 `app.rs::theme` 把 System 回落为 Dark（Iced 0.14 无系统主题探测入口）；切换入口是 `显示 › 主题` 菜单，选择写入 `settings.ron`。
- 菜单是**窗口内自绘**的（Iced 0.14 无系统菜单 API）：结构与勾选在 `menu.rs`，绘制在 `view/menu.rs`；
  Windows / Linux 的菜单栏并入自绘标题栏，macOS 单独占窗口顶部一行。新增菜单项先加 `Action` 与文案字段，
  再按需接 `update/menu.rs`，不要在 `view` 里直接写业务。
- **自绘控件要自己声明光标**：`mouse_area` 默认 `interaction: None`，不显式写 `.interaction(...)` 就沿用底层控件的
  光标；而 `stack` 合成光标时自上而下取第一个非 `None` 的结果，所以叠层（菜单面板、弹窗、遮罩）必须声明
  `Interaction::Idle` 才能挡住底部按钮的手型。可点击的行 / 卡片用 `Pointer`，禁用的条目保持 `Idle`；
  菜单栏标题（文件 / 显示 / 帮助）按原生习惯也用 `Idle`，所以它是 `container` + `mouse_area` 自绘而非 `button`，
  悬停底色由 `MenuState::bar_hover` 驱动。
- **画布只在拖动中改光标**：Windows 把 `Grab` / `Grabbing` 都映射成 `IDC_SIZEALL`（四向箭头），悬停时给出它会被
  当成错位资源，所以 `image_canvas.rs` 的 `mouse_interaction` 在画布内返回 `Idle`、按下拖动才 `Grabbing`、
  范围外交还 `None`（`column` / `row` 用 `max()` 合成子控件光标，越界不交还就会按住整个窗口）。
- 视觉规范以 [design/macos-first](design/macos-first/README.md) 为准。颜色、圆角、间距、字号只从 `theme::Palette` 与 `theme::` 样式函数取，**不要在 `view/` / `widgets/` 里硬编码颜色**；新增可复用样式加在 `theme.rs`，不要就地写匿名闭包样式。图标统一用 `widgets/icons.rs` 的内联 SVG（`Icon` 枚举 + 主题着色），不引入图标字体或图标 crate。
- 标题栏按平台分叉：macOS 保持系统原生装饰；Windows / Linux 关装饰、用 `view/titlebar.rs` 自绘（拖动、双击最大化、caption 按钮）。改标题栏或窗口 `Settings` 时两侧都要照顾，且注意全屏下自绘标题栏要隐藏。
  caption 三钮（46×28）必须贴到窗口右边缘：横向内边距只给左侧（`Padding { left: 12, right: 0 }`），
  `spacing` 只作用在标题与按钮之间，否则关闭按钮会离边 12px，看起来像没靠右。
  关闭按钮悬停时图标转白：`button::Style.text_color` 触不到内部 SVG，配色由
  `caption_icon_color()` 在构建时按 `AppState::caption_hover` 决定；该状态在
  `CursorLeft` / 窗口 `Unfocused` 时要主动复位（`update/task.rs`），否则图标会停在白色。
- 无边框窗口不会自动圆角：Windows 需要在 `window::Settings.platform_specific.corner_preference` 里显式给 `CornerPreference::Round`（Win11 生效，最大化 / 旧系统由系统忽略）。别把这个字段当噪音删掉。
- **`button` 会把内容区的 min/max 都收紧成按钮尺寸**（`layout::padded` → `Limits::width(Fixed)`），内部固定尺寸的 SVG / 图片会被 `Limits::resolve` 拉到按钮大小。图标按钮要按 `(按钮边长 - 图标边长) / 2` 设 padding 把内容区收回，否则图标会顶满按钮（曾导致 17px 图标被渲染成 30px）。文本按钮同理：不设 `padding(0)` 时默认 padding 会挤掉内容宽度，且 `Text` 默认左对齐，表现就是「符号没居中」——胶囊缩放的 `−` / 百分比 / `+` 三段必须同时 `padding(0)` + `center()`。
- **`container` 默认左上对齐**（`Horizontal::Left` / `Vertical::Top`），`container(x).width(w).height(h)` 不会把内容居中——自绘的「格子」控件（菜单栏标题、图标槽位）必须显式 `.align_x(Center).align_y(Center)`，否则文字会贴在格子左上角（曾导致菜单栏标题偏左偏上）。
- 「跟随系统」的语言判定在 `iv-domain::Language::resolve_tag`（只看主语言子标签，非中文回退英文），系统语言由 `iv-platform::system_locale()` 读取；GUI 侧由 `AppState` 启动时读一次 `system_locale`，`i18n::for_language` 据此解析。

## 本分支没有的能力

以下在主分支存在、但本分支**没有**，不要按主分支的文档去实现或声称具备：Tauri / Vue 前端、`iviewer://` 资源协议、系统原生菜单栏（菜单为窗口内自绘）、自动更新、单实例 / IPC / 文件关联、安装包与发布流程、WebView 相关的一切（CSP、Display P3 画布、16-bit 屏幕呈现）、动画逐帧播放、文件监听（`notify`）、触控板手势。需要这些时先确认是否属于本分支范围。

## 文档措辞

- 需要交代历史时用版本或时间：「1.2.0 起」「1.2.0 之前的 eframe 版」（统一写 `eframe 版`，不写 `egui 版`）；重写记录标题格式为「P1-Mx 重写：…（1.2.0，日期）」。
- 不要把主分支的 GUI 能力写成本分支的；描述本分支时用「Iced GUI」或直接陈述事实，避免含糊的「本程序」指向 Tauri 版。
- 验收记录写清环境、方法、实测数据与**未覆盖项**；没做目视确认就如实写没做。

## 测试与夹具

- 单测跟实现放同一文件；涉及磁盘的用 `std::env::temp_dir()` 下的独立目录，用完清理。
- 动画夹具：`cargo run -p iv-image-decode --example animation_fixtures`，产物在 `target/animation-fixtures/`。
- 格式样本夹具：`cargo run -p iv-image-decode --example format_fixtures`，产物在 `test_files/`。
  （1.2.0 重写前的 `gen_fixtures` / `color_fixtures` / `thumb_fixtures` 随 eframe 版一并移除，命令已不可用。）
- 依赖审计：`cargo deny check`（配置见 `deny.toml`，离线时加 `--offline`）。
