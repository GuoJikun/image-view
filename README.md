# 图片查看器（image-view）· Iced GUI 分支

一个轻量的 Rust 桌面图片查看器。本分支的 GUI 是**纯 Rust 的 [Iced](https://github.com/iced-rs/iced) 0.14**，业务逻辑由 workspace 的 `crates/iv-*` 共享。目标是把「打开图片、看清细节」这件事做快、做稳。

> **关于本分支**：这是 Iced GUI 实验分支（`gui-iced`），**不会合并回 `main`**。它定期把 `main` 的 `crates/`（业务逻辑）与工作区根 `Cargo.toml` 合并进来，GUI 层则独立维护在 `app/`。因此本分支的文档按 Iced GUI 的实际能力描述；`docs/` 下其他 GUI 方案（eframe / Tauri）的架构文档仅作设计档案保留，不代表本分支的运行形态。

## 功能

- **打开**：命令行参数、`Ctrl+O` 文件对话框、拖拽文件到窗口；命令行可传图片文件或目录（目录取其中第一张）。
- **浏览**：同目录上一张 / 下一张（`←` / `→`、`Home` / `End`），不循环；按文件名自然排序或按修改时间排序（在 `⋯` 菜单切换，选择会记住）。
- **查看**：适配窗口、100%（1 图像像素 = 1 物理屏幕像素）、以光标为锚点的滚轮缩放、拖拽平移、双击在适配 / 100% 间切换；`r` / `R` 顺时针 / 逆时针旋转 90°。
- **缩略图条**：底部虚拟滚动缩略图条，只解码可见区间（外扩 3 张）；可一键显示 / 隐藏，翻页时自动定位到当前图。
- **元数据面板**：右侧面板显示格式、文件名、尺寸、文件大小、修改时间、位深与色彩转换信息。
- **幻灯片**：`Space` 开始 / 停止，3 秒一张，到最后一张自动停止。
- **文件操作**：`Delete` 移到系统回收站、`Ctrl+Shift+C` 复制路径、在资源管理器中定位；F11 全屏。
- **会话记忆**：上次目录 / 图片、排序、缩略图条开关写入配置文件，下次启动自动恢复。

## 运行

需要 Rust 1.95+（edition 2024）：

```bash
cargo run -p image-view-iced -- "D:\photos\a.jpg"
cargo run -p image-view-iced -- "D:\photos"    # 打开目录中第一张
```

命令行参数：

```sh
image-view-iced [PATH]

  PATH            要打开的图片文件，或目录（打开其中第一张）
  -h, --help      显示帮助
  -V, --version   显示版本
```

只读取第一个位置参数，多余参数被忽略。二进制名为 `image-view-iced`，版本号继承自工作区 `Cargo.toml`（当前 1.5.0），随 `main` 同步；本分支不产出安装包。

## 配置

偏好写入 `settings.ron`，默认位置：

- Windows：`%APPDATA%\image-view-iced\`
- macOS：`~/Library/Application Support/image-view-iced/`
- Linux：`$XDG_CONFIG_HOME/image-view-iced/`（未设则 `~/.config/image-view-iced/`）

可用环境变量 `IMAGE_VIEW_ICED_CONFIG_DIR` 覆盖目录。文件采用 RON 格式、原子写入（先写临时文件再重命名），解析失败时静默回退默认值。

当前生效的字段：最近目录 / 图片、排序方式、缩略图条开关、主题。共享的 `Settings` 里还有语言、色彩转换标记、启动时检查更新等字段，但 Iced GUI 尚未接入（见「已知限制」）。

## 快捷键

| 按键 | 功能 |
| --- | --- |
| `Ctrl+O` | 打开文件 |
| `←` / `→` | 上一张 / 下一张 |
| `Home` / `End` | 第一张 / 最后一张 |
| `Ctrl+Shift+C` | 复制当前图片路径 |
| `Delete` | 移到回收站 |
| `Space` | 开始 / 停止幻灯片 |
| `+` / `=` | 放大（滚轮向上同样以光标为锚点放大） |
| `-` / `_` | 缩小 |
| `0` | 适配窗口 |
| `1` | 100% |
| `f` / `F` | 在适配与 100% 间切换（双击画布同效） |
| `r` / `R` | 顺时针 / 逆时针旋转 90° |
| `F11` | 全屏 |
| `Esc` | 关闭 `⋯` 菜单 / 清除提示横幅 |

## 支持的格式

PNG、JPEG、GIF、WebP、BMP、TIFF、ICO、QOI、TGA、PNM、HDR、EXR、DDS、farbfeld（`.ff`）、JPEG XL、AVIF、HEIC/HEIF（含 `.heif`）、SVG，共 21 个扩展名。格式识别与解码由 `crates/iv-image-decode` 提供（从 `main` 同步），HEIC / AVIF / SVG / JPEG XL 为纯 Rust 解码、默认开启。

JPEG / TIFF / PNG / WebP / JXL / HEIC / AVIF 内嵌的 ICC profile 会按 Core 的色彩链路处理；无 profile 时按 sRGB 处理。

## 已知限制

本分支是 GUI 实验分支，以下能力**没有实现**（架构文档里出现不代表本分支具备）：

- **无多语言**：界面文案固定中文，未接入 `iv-domain::Language`。
- **无原生菜单栏**：只有自绘工具栏与 `⋯` 溢出菜单。
- **无单实例 / 文件关联 / 安装包 / 自动更新**：这些属于 Tauri 主分支的发布工程，本分支不含。
- **无文件监听**：目录内容在运行期间新增 / 删除后，需重新打开目录才刷新。
- **无动画播放**：GIF / WebP / APNG 按静态首帧显示，不逐帧播放。
- **无翻转、无触控板手势**：旋转只有视图级 90° 步进，不写回文件，切换图片后重置。
- **主题**：只在读配置文件时生效（跟随系统在本分支回落为深色），界面没有切换控件；元数据面板开关也不持久化。
- **目录扫描不递归**且只在打开时执行一次。
- **Display P3 输出与 16-bit 屏幕观感**等依赖 WebView 画布的能力不适用于本分支；编码、ICC 转换与 PNG 传输环节仍保留位深，最终由 Iced 的 wgpu renderer 以常规精度呈现。

## 项目结构

```plain
Cargo.toml              cargo workspace：成员、共享依赖、release profile 与版本号
app/                    Iced GUI：入口、状态、消息、Update/View、自绘 widgets 与适配器
  Cargo.toml            GUI 包声明（image-view-iced）与 Iced 依赖/特性
  src/
    main.rs             CLI、日志、窗口图标与 Application 构建
    app.rs              boot / update / view / subscription / title / theme 装配
    message.rs          UI 事件与异步结果（不进入 Core）
    state/              顶层状态、查看器状态、图库/元数据状态
    update/             Message → Core 用例的映射与周期轮询
    view/               工具栏、画布区、状态栏、溢出菜单
    widgets/            图片画布、缩略图条、元数据面板
    adapters/           预览/缩略图服务、RON 存储、错误文案
crates/                 与 GUI 无关的业务逻辑（从 main 同步），按依赖方向分层
  iv-domain/            排序、语言、主题与支持的扩展名
  iv-files/             目录扫描与自然/时间排序
  iv-application/       目录导航状态（上一张/下一张/选中）
  iv-metadata/          ICC profile 解析、sRGB 转换与 16-bit 精度
  iv-image-decode/      后台解码、优先级队列、预加载、降采样与缩略图
  iv-image-render/      缩放/平移视图数学（不依赖 GUI）
  iv-cache/             LRU 与像素预算缓存、磁盘缩略图缓存
  iv-task/              后台请求代号（generation）
  iv-platform/          回收站删除、资源管理器定位与打开目录
  iv-infra/             日志轮转、设置持久化与存储端口
  iv-gui-contract/      GUI 动作与工具栏状态（本 GUI 未使用）
  iv-library/           图库能力占位（尚未实现）
  iv-image-process/     图片处理能力占位（尚未实现）
.github/workflows/      CI 配置
docs/                   路线图、分阶段计划与架构说明
design/                 UI 设计规范
CHANGELOG.md           各版本的用户可见变更
```

根 `Cargo.toml` 另有 `rust-toolchain.toml`（工具链钉版）与 `deny.toml`（依赖审计）。架构与设计取舍见 [docs/02-iced-architecture.md](docs/02-iced-architecture.md)，编码约定见 [AGENTS.md](AGENTS.md)。

## 开发

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

动画 / 格式验收夹具由 `iv-image-decode` 的 example 生成：

```bash
cargo run -p iv-image-decode --example animation_fixtures
cargo run -p iv-image-decode --example format_fixtures
```

产物位于 `target/animation-fixtures/` 与 `test_files/`。依赖审计用 `cargo deny check`（离线加 `--offline`）。

## 文档索引

- [docs/ROADMAP.md](docs/ROADMAP.md)：进度总览（按本分支口径）
- [docs/02-iced-architecture.md](docs/02-iced-architecture.md)：Iced + Rust 架构与本分支实施记录
- [docs/01-eframe-egui-architecture.md](docs/01-eframe-egui-architecture.md)、[docs/03-tauri-vue-rust-architecture.md](docs/03-tauri-vue-rust-architecture.md)：其他 GUI 方案的设计档案（本分支不采用）
- [docs/PLAN-P0.md](docs/PLAN-P0.md) ~ [docs/PLAN-P3.md](docs/PLAN-P3.md)：各阶段计划与验收记录
