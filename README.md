# 图片查看器（image-view）

一个轻量的 Rust 桌面图片查看器，基于 **eframe/egui 0.37**。目标是把「打开图片、看清细节」这件事做快、做稳。

## 功能

- 通过文件对话框（Ctrl+O）、命令行参数或拖拽打开图片
- 「适配窗口」与「100%」（1 图像像素 = 1 物理屏幕像素）
- 自动按 EXIF 方向校正（JPEG/TIFF/WebP），HEIC/AVIF 应用容器内的旋转/镜像属性；`R` / `Shift+R` 顺时针 / 逆时针旋转 90°
- 读取 JPEG/TIFF/PNG/WebP/HEIC/AVIF 内嵌 ICC profile 并转换到 sRGB；16-bit PNG/TIFF 在转换与缩放阶段保持精度，最终以 8-bit 纹理显示
- 以光标为锚点的滚轮 / 触控板缩放，鼠标拖拽平移，双击在「适配」与「100%」之间切换
- 同一目录内上一张 / 下一张（按文件名自然排序，不循环），可切换按修改时间排序
- 底部缩略图条（可一键显示/隐藏）：懒加载 + 虚拟滚动，点击跳转，方向键/点击后自动居中当前项；缩略图使用独立内存 LRU（512 张、最长边 128 px）与磁盘缓存（`%LOCALAPPDATA%\image-view\thumbs`，上限 256 MB）
- 记住窗口位置/大小/最大化、上次目录与图片、排序方式、主题与缩略图条开关；无参数启动时恢复上次会话
- `Delete` 删除到系统回收站，`Ctrl+Shift+C` 复制路径，`⋯` 菜单或右键菜单可在资源管理器中定位
- 文件日志与崩溃转储落盘（`%LOCALAPPDATA%\image-view\logs`），`⋯ → 打开日志文件夹`可直接定位
- 幻灯片：按 `Space` 每 3 秒自动前进，到最后一张自动停止
- 相邻图片预加载 + 最近使用（LRU）主图纹理缓存，最多 3 张，回翻立即显示；缩略图预取当前项前后各 3 张
- 主图与缩略图纹理使用 mipmap + LINEAR：缩小查看更平滑，100% 附近仍保持清晰
- 26 px 单行状态栏：原图尺寸/显示尺寸与位深、格式、序号、加载与降采样提示；可在 `⋯` 菜单开启色彩转换标记
- 52 px 自绘工具栏：图标按钮、`⋯` 溢出菜单、可切换的 88 px 缩略图条；空状态与内联错误横幅
- 后台单线程解码，打开请求优先于预加载；快速翻页时只保留最新请求，旧结果自动丢弃；缩略图使用独立后台线程，避免阻塞主图
- 大图安全阈值：单边 8192、总像素 40MP 自动等比降采样；超过 250MP 的原图直接拒绝解码；预加载只处理 30MP 以下的图片
- 界面支持简体中文 / English（可跟随系统），在 `⋯ → 语言` 中切换并立即生效；主题可选跟随系统 / 浅色 / 深色，画布在两套主题下都保持深色，保证图片显示时始终有最大对比度
- 自动更新：`⋯ → 检查更新…` 读取 GitHub Releases 的 `update.json`，下载后校验 SHA-256 再启动静默安装；启动时检查默认关闭，可手动开启

## 版本与路线

当前版本为 **v1.3.0（开发中）**。`1.0.0` 汇总了 P0 查看基线、P1-M1（桌面集成与持久化）、P1-M2（ICC 色彩管理与 16-bit）、P1-M3（HEIC / AVIF）、P1-M4（缩略图条与渲染质量）、P1-M5（发布工程：安装器、文件关联、崩溃日志、版本规范）、界面改版（版式 A）、P1-M8（自动更新）与 P1-M9（中/英多语言），默认构建包含 HEIC/AVIF/SVG 解码器，渲染后端为 glow。`1.2.0` 增加 Debian/Ubuntu 打包，`1.3.0` 增加 macOS（Apple Silicon）打包；图库/搜索/对比（M6）与 GIF/WebP 动画（M7）将在后续版本提供。

- 阶段划分与进度：[docs/ROADMAP.md](docs/ROADMAP.md)
- v1.0 计划：[docs/PLAN-P1.md](docs/PLAN-P1.md)
- v2.x / v3+ 方向：[docs/PLAN-P2.md](docs/PLAN-P2.md) · [docs/PLAN-P3.md](docs/PLAN-P3.md)
- 1.0.0 开发基线（P0）设计与验收：[docs/PLAN-P0.md](docs/PLAN-P0.md)

## 安装与卸载

### Windows（10/11 x64）

Windows 10/11 x64 用 cargo-packager 与 NSIS 生成安装包：

```powershell
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1
```

产物位于 `dist\`：`image-view_<version>_x64-setup.exe`、`update.json` 与 `SHA256SUMS.txt`。把安装包和 `update.json` 一起上传到 GitHub Release，应用即可通过 `releases/latest/download/update.json` 发现新版本。

推送版本标签会触发 GitHub Actions 发布（`.github/workflows/release.yml`）：

```powershell
git tag v1.0.0
git push origin v1.0.0
```

工作流只接受以 `v` 开头且与 `Cargo.toml` / `Packager.toml` 版本一致的标签（例如 `v1.0.0`），随后构建安装包、`update.json` 与校验文件，并自动创建 GitHub Release。

- **安装**：按用户安装到 `%LOCALAPPDATA%\image-view`，无需管理员权限；创建开始菜单快捷方式，静默安装还会创建桌面快捷方式。
- **文件关联**：注册 PNG、JPEG、GIF、WebP、BMP、TIFF、ICO、QOI、TGA、PNM、HDR、EXR、DDS、AVIF、HEIC/HEIF、SVG 的“打开方式”条目，并登记 Windows“默认应用”能力项。安装器不会改写任何扩展名的当前默认处理器（Windows 10/11 会阻止并回滚这类静默修改），需要设为默认时请在“设置 → 默认应用”中选择 image-view。
- **卸载**：从“设置 → 应用”或开始菜单卸载，会删除程序文件、快捷方式、卸载项与文件关联注册。程序目录同时存放用户数据（`logs\` 日志、`thumbs\` 缩略图缓存），卸载默认保留；勾选卸载页的“删除应用数据”会一并清除，设置文件 `%APPDATA%\image-view` 同理由该选项控制。
- **签名**：安装包与 exe 尚未做代码签名，SmartScreen 可能提示“未知发布者”；校验文件完整性请对照 `SHA256SUMS.txt`。

### Linux（Debian/Ubuntu）

`.deb` 必须在 Linux 环境中构建（Windows 版 cargo-packager 不包含 deb 后端）。WSL2 下进入仓库后运行：

```bash
bash scripts/build-deb.sh
```

产物为 `dist/image-view_<version>_amd64.deb`。依赖、桌面入口与文件关联位于 `Packager.toml` 的 `[deb]` / `[[fileAssociations]]`，完整步骤见 [docs/BUILD-DEB.md](docs/BUILD-DEB.md)。

### macOS（Apple Silicon）

仅支持 arm64，需在 Apple Silicon Mac 上构建（不提供 Intel / Universal 版本）。在仓库根目录运行：

```bash
bash scripts/build-macos.sh
```

产物为 `dist/image-view_<version>_aarch64.dmg`，内含 `image-view.app`。最低系统版本、DMG 窗口布局与文件关联位于 `Packager.toml` 的 `[macos]` / `[dmg]` / `[[fileAssociations]]`；签名、公证与验证步骤见 [docs/BUILD-MACOS.md](docs/BUILD-MACOS.md)。未签名时首次打开需在 Finder 中右键应用选择“打开”；文件关联已注册，但 Finder 双击传入文件的 macOS 事件尚未处理，需从命令行传入路径。

## 自动更新

- 入口在 `⋯ → 检查更新…`；同一菜单的“启动时检查更新”默认关闭，勾选后才会在启动时联网检查。
- 默认清单地址为 `https://github.com/GuoJikun/image-view/releases/latest/download/update.json`，格式为 `{"version","url","sha256","notes"}`。可用环境变量 `IMAGE_VIEW_UPDATE_MANIFEST` 指向本地文件或测试地址，用 `IMAGE_VIEW_UPDATE_DIR` 覆盖下载目录。
- 安装包分块下载到 `%LOCALAPPDATA%\image-view\updates`，先写 `.part`；SHA-256 与清单一致后才会重命名并允许安装。安装器以 `/S` 静默运行，应用随即退出，不会自动重新启动。
- 无网络、清单损坏或哈希不匹配时只影响更新窗口，不影响正常看图。

## 运行

需要 Rust 1.95+（edition 2024）。

```powershell
cargo run --release
cargo run --release -- "D:\photos\a.jpg"
cargo run --release -- "D:\photos"      # 打开目录中第一张图片
```

构建产物位于 `target\release\image-view.exe`，可自行复制到任意位置使用。

HEIC/AVIF 与 SVG 解码为可选特性，默认开启，且均为纯 Rust 实现，不需要额外的 C 运行库或 DLL：

```powershell
cargo build --release --no-default-features                     # 不含 HEIC/AVIF/SVG
cargo build --release --no-default-features --features avif     # 仅 AVIF
cargo build --release --no-default-features --features heic     # 仅 HEIC/HEIF
cargo build --release --no-default-features --features svg      # 仅 SVG（resvg 光栅化）
```

未包含对应解码器的构建打开这类文件时，会显示“解码器未包含在此构建中”的明确提示，不会崩溃。

命令行参数：

```
image-view [PATH]

  PATH            要打开的图片文件，或目录（打开其中第一张）
  -h, --help      显示帮助
  -V, --version   显示版本
```

日志默认写入 `%LOCALAPPDATA%\image-view\logs\image-view.log`（可用 `IMAGE_VIEW_LOG_DIR` 覆盖），同时输出到 stderr；单文件超过 4 MiB 自动轮转，保留 3 份历史。可通过环境变量 `RUST_LOG` 控制级别；`RUST_LOG=image_view=debug` 可查看预加载、缓存命中等细节。崩溃时会在同目录生成 `crash-*.log`（含 backtrace，最多 10 份），可用“文件 → 打开日志文件夹”直接定位。

## 隐私

应用不包含遥测、统计或自动联网上报；日志与崩溃转储只写入本机 `%LOCALAPPDATA%\image-view\logs`。自动更新默认不在启动时检查；只有点击 `⋯ → 检查更新…` 或勾选“启动时检查更新”后，才会访问 GitHub Releases 读取 `update.json`，且不会发送任何使用数据。

## 快捷键

| 按键 | 功能 |
| --- | --- |
| `Ctrl+O` | 打开文件 |
| `←` / `→` | 上一张 / 下一张（缩略图条同步居中） |
| `Ctrl+Shift+C` | 复制当前图片路径 |
| `Delete` | 移到回收站 |
| `Space` | 开始 / 停止幻灯片（3 秒间隔） |
| `+` / `-`（或滚轮） | 放大 / 缩小 |
| `0` | 适配窗口 |
| `1` | 100% |
| `F` | 在「适配」与「100%」之间切换 |
| 双击画布 | 同上 |
| `F11` | 全屏 |
| `R` | 顺时针旋转 90° |
| `Shift+R` | 逆时针旋转 90° |
| `Esc` | 退出全屏 / 关闭帮助窗口 |

光标位于缩略图条时，滚轮横向滚动缩略图；点击缩略图直接跳转到对应图片。

## 支持的格式

PNG、JPEG、GIF、WebP、BMP、TIFF、ICO、QOI、TGA、PNM、HDR、EXR、DDS、AVIF、HEIC/HEIF、SVG。

HEIC/AVIF 默认启用，使用纯 Rust 解码器（`imgalifier-heif` / `imgalifier-avif`），不依赖 libheif 或系统 DLL。`.heif` 扩展名按 HEIC 处理。

SVG 默认启用，使用 `resvg`（usvg + tiny-skia）光栅化，同样为纯 Rust、无系统 DLL：先解析出 `width`/`height`（缺失时回退 `viewBox`，两者都没有时按规范用 300×150）并做 250 MP 原图上限检查，再直接渲染到目标尺寸——矢量缩放即渲染，不会先画成位图再重采样。`.svgz`（gzip 压缩）暂不支持。

JPEG/TIFF/PNG/WebP/HEIC/AVIF 的内嵌 ICC profile 会转换到 sRGB；无 profile 时按 sRGB 处理。CMYK 专用 profile 不受支持（解码后已转为 RGB），会回退原像素显示。

## 已知限制

- EXIF 方向校正只覆盖 JPEG/TIFF/WebP；HEIC/AVIF 使用容器内的旋转/镜像属性（`irot` / `imir`）。
- HEIC/AVIF 若只带 nclx 色彩原色（如未内嵌 ICC 的 Display P3），暂不进行色域转换；内嵌 ICC 会经 LCMS 转换到 sRGB。iPhone 照片通常内嵌 ICC。
- HEIC/AVIF 的 10/12-bit 源图由解码器内部转换为 8-bit 纹理；状态栏位深标记只区分通用解码路径的 8/16-bit。
- HEIC（HEVC/H.265）涉及专利许可，发布或商业分发前需自行评估；AVIF（AV1）不在此列。
- SVG 只渲染静态内容：不支持动画、脚本与事件；文字使用系统字体，缺少指定字体时回退到可用字体，嵌入式 Web 字体不加载。
- SVG 按声明的固有尺寸（`width`/`height`/`viewBox`，逻辑像素）做上限检查，超过 250 MP 会被拒绝打开；`.svgz`（gzip 压缩）不在支持列表内。
- 旋转仅作用于当前查看，不写回文件，切换图片后重置。
- 动图只显示第一帧。
- 超大图会被降采样后再显示，此时「100%」是相对降采样后的纹理，状态栏会同时标注原图尺寸。
- 目录扫描不递归，且在打开文件时一次性完成；运行期间新增的文件需重新打开目录才可见。
- 设置与窗口状态保存在 `%APPDATA%\image-view\data\app.ron`；文件损坏或版本过旧时自动回退默认值。
- 排序按修改时间读取文件元数据；网络盘或权限受限时，无法读取时间的文件排在最后。
- 删除只支持系统回收站；回收站不可用（如部分网络盘）时会提示错误并保留原文件。
- 主图纹理缓存最多保留 3 张图片（总像素 ≤ 72M，约 288 MB）；缩略图另有 512 张 LRU，与主图预算分离。
- 缩略图磁盘缓存位于 `%LOCALAPPDATA%\image-view\thumbs`，按路径、修改时间、文件长度与目标尺寸缓存 QOI；超过 256 MB 时启动阶段删除最旧文件，可安全手动清空。
- 缩略图最长边为 128 px，适合 2× DPI 与常规缩放；极大目录下缩略图条只绘制视口附近卡片，但目录扫描本身仍是一次性完成。
- mipmap 会增加约 33% 纹理显存；当前主图与缩略图都启用 mipmap + LINEAR。glow（OpenGL）为主要渲染后端，老显卡驱动异常时需回退到旧版本或自行切换后端。
- 幻灯片到最后一张停止，不循环；手动翻页会重置 3 秒计时。
- 预加载会跳过超过 30 MP 的图片（仅影响速度，不影响正常打开）。
- 16-bit 图片以 8-bit 纹理显示（色彩转换阶段仍保留 16-bit 精度）；10-bit/HDR 输出不在当前版本范围内。
- ICC 转换不支持 CMYK 专用 profile；profile 损坏或与图像通道不匹配时按原像素显示，具体原因写入 debug 日志。
- 安装包与 exe 尚未签名，SmartScreen 可能提示“未知发布者”；首版接受该提示，后续版本再补代码签名。
- Windows 10/11 的默认应用由系统 `UserChoice` 保护，安装器无法静默夺取常见扩展名的默认查看器；请在“设置 → 应用 → 默认应用”中确认。
- 安装向导支持英文 / 简体中文；应用内界面也可在“跟随系统 / 简体中文 / English”之间切换。自动更新只做 SHA-256 校验、没有代码签名；网络受限或需要回滚时，请从发布页手动下载安装包覆盖安装。
- 日志与崩溃转储会写入 `%LOCALAPPDATA%\image-view\logs`；卸载会一并删除。若需长期保留问题现场，请在卸载前复制该目录。

## 项目结构

```
src/
  main.rs       入口：CLI、窗口参数、图标、日志初始化
  app.rs        应用状态与每帧编排（eframe::App 的 logic / ui）
  ui.rs         顶部工具栏、分段缩放、溢出菜单、状态栏、错误横幅、帮助窗口
  theme.rs      浅/深色调色板、主题设置与 egui 样式安装
  icons.rs      工具栏与空状态使用的矢量图标绘制
  viewer.rs     缩放/平移数学与画布绘制
  document.rs   当前图片与 GPU 纹理
  cache.rs      最近使用（LRU）纹理缓存
  loader.rs     后台解码线程、优先级队列、预加载、阈值检查、降采样、HEIC/AVIF/SVG 分派、SVG 光栅化、缩略图尺寸解码
  color.rs      ICC profile 解析、sRGB 转换与 16-bit 精度保持
  thumbs.rs     缩略图条、虚拟滚动、内存 LRU、磁盘缓存与后台缩略图解码
  folder.rs     目录扫描、自然排序/时间排序、上一张/下一张、缩略图选中
  settings.rs   持久化设置：窗口状态、上次会话、排序、主题、语言与更新偏好
  fileops.rs    回收站删除、资源管理器定位与打开目录
  logging.rs    文件日志轮转、启动信息与 panic 崩溃转储
  i18n.rs       中/英资源表、语言检测与切换
  update.rs     更新清单检查、后台下载、SHA-256 校验与安装器启动
build.rs        在 Windows MSVC 下把 assets/app.res 链接进 exe
examples/
  gen_fixtures.rs    生成手工测试用的图片夹具（开发用）
  color_fixtures.rs  生成 AdobeRGB / Display P3 / sRGB 色彩验收夹具
  thumb_fixtures.rs  生成缩略图条 1000 / 3000 张目录验收夹具
assets/
  icon.png      窗口图标（64×64）
  icon-256.png  deb / macOS 打包用的 256×256 图标
  icon-512.png  deb / macOS 打包用的 512×512 图标
  icon-512@2x.png macOS Retina 图标（1024×1024，按 512 pt @2x 命名）
  icon.ico      多尺寸应用图标（安装包 / exe）
  app.rc        图标资源定义
  app.res       编译后的 Windows 资源
installer/
  nsis/
    image-view.nsi   NSIS 安装器模板（按用户安装、文件关联注册、卸载清理）
Packager.toml       cargo-packager 打包配置（NSIS / deb / dmg、依赖与文件关联）
scripts/
  build-release.ps1  构建 release exe、安装包、update.json 与 SHA256
  build-deb.sh       在 Linux/WSL2 中构建 .deb
  build-macos.sh     在 Apple Silicon Mac 上构建 arm64 .dmg
.github/
  workflows/
    release.yml      推送 v* 标签时自动构建并发布
design/
  gen_ico.py    从 icon-1024.png 生成多尺寸 assets/icon.ico
docs/
  ROADMAP.md    阶段进度总览与里程碑
  PLAN-P0.md    1.0.0 开发基线（P0）设计与验收记录
  PLAN-P1.md    v1.0 计划（M1~M9，只查看）
  PLAN-P2.md    v2.x 专业能力、编辑与架构
  PLAN-P3.md    v3+ 长期方向
  NSIS-TOOLCHAIN.md  cargo-packager 的 NSIS 工具链预置与排障
  BUILD-DEB.md       WSL2/Linux 构建 .deb 的依赖、步骤与配置说明
  BUILD-MACOS.md     Apple Silicon 构建 arm64 .dmg 的步骤与签名说明
CHANGELOG.md    各发布版本的用户可见变更
```

## 开发

```powershell
cargo fmt
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
powershell -ExecutionPolicy Bypass -File scripts\build-release.ps1   # exe + 安装包 + SHA256
```

`build-release.ps1` 需要已安装 `cargo-packager`（`cargo install cargo-packager --locked`），并已按 [docs/NSIS-TOOLCHAIN.md](docs/NSIS-TOOLCHAIN.md) 预置 NSIS 工具链（`%LOCALAPPDATA%\.cargo-packager`）。发布前还应运行 `cargo fmt --check` 与 `cargo test --no-default-features`。

手工测试夹具可用 `cargo run --release --example gen_fixtures -- <输出目录>` 生成（会写入约 100MP 的 JPEG，注意磁盘空间）。色彩验收夹具用 `cargo run --release --example color_fixtures -- <输出目录>` 生成。缩略图验收夹具用 `cargo run --release --example thumb_fixtures -- <目录> [数量] [宽] [高]` 生成（例如 3000 张 640×480）。

- 设计细节与取舍见 `docs/PLAN-P0.md`。
- 渲染后端使用 eframe 的 glow（OpenGL）；从 wgpu 切换是为了支持纹理 `mipmap_mode`，同时减小 release 体积。
