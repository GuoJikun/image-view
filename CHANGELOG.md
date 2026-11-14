# 变更记录

本文件只记录各发布版本的用户可见变更，格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循[语义化版本](https://semver.org/lang/zh-CN/)，发布时打标签 `vX.Y.Z`。

## [未发布]

### 新增

- 恢复 1.2.0 移除的图片格式「打开方式」注册（改为打包声明式）：19 个扩展名（`png` / `jpg` / `jpeg` / `gif` / `webp` / `bmp` / `tif` / `tiff` / `ico` / `qoi` / `tga` / `pnm` / `hdr` / `exr` / `dds` / `avif` / `heic` / `heif` / `svg`）注册 ProgID `ImageView.<ext>`、资源管理器「类型」列名称与打开命令；安装前的默认处理器自动备份，卸载时还原。
- Windows 安装包补齐协助程序声明：`OpenWithProgids`、`Applications\image-view.exe\SupportedTypes` 与 `RegisteredApplications` + `Capabilities` 能力项，本程序始终出现在右键「打开方式」与「设置 → 默认应用」中，不再依赖是否持有默认处理器；卸载时清理上述键值。
- macOS 与 Linux 同步声明文档类型（`CFBundleDocumentTypes` / `.desktop` 的 `MimeType`），三端「打开方式」菜单均可列出本程序。

## [1.3.0] - 2026-11-14

统一安装与运行标识，日志改为随安装目录存放；从 1.2.0 升级需先卸载旧版。

### 变更

- 安装与运行标识统一为 `image-view`：Cargo 包名、`productName` 与 bundle identifier（`dev.jikun.imageview`）同步调整。Windows 安装目录变为 `%LOCALAPPDATA%\image-view`，安装包文件名同步变化；从 1.2.0 升级前请先卸载旧版本，否则会并存两条卸载记录。
- 日志与崩溃转储改为写入安装目录的 `logs\`，随安装目录一并卸载清理；安装位置不可写时（系统级安装、AppImage、macOS `.app`）自动回退到用户数据目录。

## [1.2.0] - 2026-11-14

GUI 全面迁移至 Tauri 2 + Vue 3，界面按「macOS 优先、三端和谐」规范重做；新增 macOS 支持与 Windows arm64 安装包，Linux 提供 DEB / RPM / AppImage。

### 新增

- macOS 支持：提供 `.dmg` 安装包，Apple Silicon（arm64）与 Intel（x64）独立构建。
- Windows arm64 安装包（NSIS）。
- Linux 新增 `.deb`、`.rpm`、`.AppImage` 安装包。
- 单实例：程序已运行时再次打开图片（命令行参数或文件管理器），会转交给已运行的窗口并聚焦，不再重复开窗。

### 变更

- GUI 从 egui 迁移到 Tauri 2 + Vue 3，界面全面重做；解码、色彩管理与目录导航继续由共享 crates 提供。macOS 使用原生标题栏与系统菜单栏，Windows / Linux 使用自绘标题栏与 VS Code 式菜单栏（文件 / 显示 / 帮助），三端菜单内容一致，窗口控件、间距与配色按平台习惯对齐；原「⋯」溢出菜单的条目收入这三个菜单。
- 自动更新迁移至 Tauri 官方 `tauri-plugin-updater`，签名校验由 SHA-256 清单改为 minisign：「帮助 → 检查更新…」查看发布说明后下载，进度与字节数实时显示、可随时取消，完成后点「安装并重启」升级。
- 安装包改由 Tauri bundler 统一产出（NSIS / DMG / DEB / RPM / AppImage）；推送 `v*` 标签后，GitHub Actions 在 Windows x64/arm64、macOS arm64/x64 与 Linux 并行构建，自动生成更新清单并创建 GitHub Release。

### 移除

- egui 版的以下能力在这次 GUI 重写中暂未带上：
  - 安装包注册图片格式「打开方式」菜单（18 种格式的文件关联）；
  - 缩略图磁盘缓存（内存 LRU 512 张仍在，进程重启后缩略图需重新解码）；
  - 记忆窗口位置/大小/最大化、恢复上次目录/图片，以及排序方式、缩略图条开关的持久化（主题与语言仍会记忆）。

## [1.1.0] - 2026-11-12

更新体验改进：发布说明按 Markdown 渲染，下载支持系统代理与取消，更新窗口更紧凑。

### 新增

- 发布说明按 Markdown 渲染：标题、列表、行内代码与链接正常显示，悬停链接可查看完整地址。
- 更新下载支持系统代理：读取 Windows 代理设置，`HTTPS_PROXY` / `HTTP_PROXY` 等环境变量优先。
- 下载中可点「取消」：立即回到发现新版本状态，并清理未完成的临时文件。

### 改进

- 下载进度条显示轨道背景，深色主题下进度更清晰。
- 更新窗口：收紧内容留白、高度贴合内容，去掉下载按钮下方的多余空白；动作按钮靠右，下载使用主色。
- 发布说明区限高并可滚动，长列表不再撑高窗口。

## [1.0.0] - 2026-11-12

首个正式版：图片查看、格式与色彩支持、安装包、自动更新与中英界面。

### 新增

- 打开图片：文件对话框（`Ctrl+O`）、命令行参数、拖拽；目录按自然顺序浏览，上一张/下一张不循环。
- 查看操作：适配窗口、100%、1%~3200% 缩放，以光标为锚点的缩放，拖拽平移，双击切换，全屏，幻灯片（`Space`，3 秒）与相邻图片预加载。
- EXIF 方向自动校正，`R` / `Shift+R` 旋转 90°（不改写文件）。
- 大图保护：单边 8192、40 MP 纹理上限、250 MP 拒绝打开，自动降采样并在状态栏提示。
- 缩略图条：懒加载、虚拟滚动、内存 LRU（512 张）与磁盘缓存（上限 256 MB），可一键显示/隐藏。
- 格式：PNG、JPEG、GIF、WebP、BMP、TIFF、ICO、QOI、TGA、PNM、HDR、EXR、DDS、AVIF、HEIC/HEIF、SVG，其中 HEIC/AVIF 与 SVG 为纯 Rust 解码。
- 色彩：读取内嵌 ICC profile 并转换到 sRGB，16-bit PNG/TIFF 保持精度，状态栏显示尺寸、位深、格式与降采样提示。
- 桌面集成：记住窗口、上次目录/图片、排序、主题、语言与缩略图条开关；删除到回收站、复制路径、资源管理器定位。
- 主题：跟随系统/浅色/深色，画布固定深色以保证图片对比度。
- 日志：`%LOCALAPPDATA%\image-view\logs`，4 MiB 轮转保留 3 份，panic 额外生成 `crash-*.log`。
- 安装包：NSIS（cargo-packager + NSIS 3.09），按用户安装到 `%LOCALAPPDATA%\image-view`，中英双语向导，注册 18 种格式的“打开方式”，卸载可选删除用户数据。
- 自动更新：`⋯ → 检查更新…`，语义化版本比较、下载进度、SHA-256 校验后静默安装；启动检查默认关闭。
- 中英界面：工具栏、菜单、状态栏、错误提示与更新窗口均本地化，切换立即生效并随设置持久化。
