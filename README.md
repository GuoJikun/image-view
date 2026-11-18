# 图片查看器（image-view）

一个轻量的 Rust 桌面图片查看器，基于 **Tauri 2 + Vue 3**。目标是把「打开图片、看清细节」这件事做快、做稳。

## 功能

**打开与浏览**

- 文件对话框（`Ctrl+O`）、命令行参数或拖拽打开；同目录上一张 / 下一张，默认按文件名自然排序，也可按修改时间
- 底部缩略图条（可一键隐藏），点击即跳转，方向键切换时自动居中；相邻图片后台预加载，回翻立即可见

**查看**

- 适配窗口与 100%（1 图像像素 = 1 物理屏幕像素）；以光标为锚点缩放、拖拽平移、双击切换
- 按 EXIF 方向自动校正，`R` / `Shift+R` 顺时针 / 逆时针旋转 90°
- 幻灯片（`Space`，每 3 秒）与 APNG / GIF / WebP 动画播放，`Space` 暂停或继续
- 超大图自动等比降采样并在状态栏提示，极端尺寸直接拒绝打开，不会卡住界面

**格式与色彩**

- 18 种常见格式，其中 APNG / GIF / WebP 支持多帧动画，HEIC / AVIF / SVG / JPEG XL 为纯 Rust 解码，详见下方「支持的格式」
- 读取内嵌 ICC profile 转换到显示目标空间：广色域屏且引擎支持 `display-p3` 画布时转到 Display P3，其余情况转 sRGB；16-bit 图像在转换与缩放阶段保持精度；状态栏显示位深、格式与色彩转换标记

**界面与设置**

- 简体中文 / English（可跟随系统）切换即时生效；主题可选跟随系统 / 浅色 / 深色，画布在两套主题下都保持深色
- 语言、主题、缩略图条与「启动时检查更新」等偏好，连同窗口位置、上次打开的目录与图片一并保留

**桌面集成与维护**

- `Delete` 删除到系统回收站，`Ctrl+Shift+C` 复制路径，可在文件管理器中定位
- 日志与崩溃转储随安装目录存放，可从「帮助」直接打开；「检查更新…」读取 GitHub Releases，校验 SHA-256 后静默安装，启动时检查默认关闭

## 版本与路线

当前版本为 **v1.5.0**。GUI 采用 Tauri 2 + Vue 3 架构，业务逻辑由 workspace crates 共享。`1.0.0` 汇总了 P0 查看基线、P1-M1（桌面集成与持久化）、P1-M2（ICC 色彩管理与 16-bit）、P1-M3（HEIC / AVIF）、P1-M4（缩略图条与渲染质量）、P1-M5（发布工程：安装器、文件关联、崩溃日志、版本规范）、界面改版（版式 A）、P1-M8（自动更新）与 P1-M9（中/英多语言），默认构建包含 HEIC/AVIF/SVG 解码器。`1.2.0` 增加 Linux（deb / rpm / appimage）与 macOS 打包、Tauri 迁移；`1.3.0` 统一安装与运行标识（Windows 安装目录改为 `%LOCALAPPDATA%\image-view`，从 1.2.0 升级需先卸载旧版），日志改为随安装目录存放；`1.4.0` 补齐 Tauri 迁移期间丢失的桌面集成能力（窗口与会话记忆、缩略图磁盘缓存、相邻图片预加载、图片格式「打开方式」注册），缩略图条改用自绘悬浮滚动条；`1.4.1` 修复更新说明显示为纯文本的问题；`1.5.0` 补齐 P1-M7（GIF / WebP 动画逐帧播放），界面偏好统一存进 `settings.ron`，并让 macOS 原生菜单显示勾选状态；图库/搜索/对比（M6）将在后续版本提供。

- 阶段划分与进度：[docs/ROADMAP.md](docs/ROADMAP.md)
- v1.0 计划：[docs/PLAN-P1.md](docs/PLAN-P1.md)
- v2.x / v3+ 方向：[docs/PLAN-P2.md](docs/PLAN-P2.md) · [docs/PLAN-P3.md](docs/PLAN-P3.md)
- 1.0.0 开发基线（P0）设计与验收：[docs/PLAN-P0.md](docs/PLAN-P0.md)

## 安装与卸载

### Windows（10/11 x64 / arm64）

用 Tauri 与 NSIS 生成安装包：

```powershell
cd app
pnpm install
pnpm tauri build
```

产物位于 `target\release\bundle\nsis\`（workspace 共享根目录的 `target`）。推送版本标签会触发 GitHub Actions 自动构建并发布（`.github/workflows/release.yml`）：

```powershell
git tag v1.5.0
git push origin v1.5.0
```

工作流会校验标签与 `Cargo.toml` / `tauri.conf.json` 版本一致（例如 `v1.5.0`），随后在 Windows x64/arm64、macOS arm64/x64、Linux 三平台并行构建，签名更新产物并自动创建 GitHub Release。

Release 正文取自 `CHANGELOG.md` 中 `## [版本号]` 段落（到下一个 `## [` 为止），并追加三端安装包的选择说明。该段落缺失时工作流会在校验阶段直接失败，因此打标签前需先把这个版本的变更记录补进 `CHANGELOG.md`。

- **安装**：按用户安装到 `%LOCALAPPDATA%\image-view`，无需管理员权限；NSIS 安装界面支持简体中文 / English，安装时可选择语言。
- **文件关联**：安装时注册 21 个图片扩展名（`png` / `jpg` / `jpeg` / `gif` / `webp` / `bmp` / `tif` / `tiff` / `ico` / `qoi` / `tga` / `pnm` / `hdr` / `exr` / `dds` / `ff` / `jxl` / `avif` / `heic` / `heif` / `svg`）的 ProgID `ImageView.<ext>` 与打开命令，并写入 `OpenWithProgids`、`Applications\image-view.exe\SupportedTypes` 与 `RegisteredApplications` 能力项，因此程序会同时出现在右键「打开方式」与「设置 → 默认应用」中；安装前的默认处理器会备份，卸载时还原并清理上述键值。
- **卸载**：从「设置 → 应用」或开始菜单卸载。

### macOS（arm64 / x64）

在 Apple Silicon Mac 上构建：

```bash
cd app
pnpm install
pnpm tauri build -- --target aarch64-apple-darwin
```

产物为 `.dmg` 安装包。Intel Mac 用 `--target x86_64-apple-darwin`。

### Linux（Debian/Ubuntu/RPM/AppImage）

```bash
cd app
pnpm install
pnpm tauri build
```

自动产出 `.deb`、`.rpm` 与 `.AppImage` 三种格式。

## 自动更新

- 入口在「帮助 → 检查更新…」；同一菜单的"启动时检查更新"默认关闭，勾选后才会在启动时联网检查。
- 基于 Tauri 官方 `tauri-plugin-updater`，更新清单地址为 `https://github.com/GuoJikun/image-view/releases/latest/download/latest.json`。
- 更新包使用 minisign 签名校验，下载后自动安装；支持下载进度显示与取消。

## 运行

需要 Rust 1.95+（edition 2024）、Node.js 22+ 与 pnpm。

```powershell
cd app
pnpm install
pnpm tauri dev          # 开发模式（自动启动 Vite + 热更新）
pnpm tauri build        # 构建 release + 安装包
```

构建产物位于 `target\release\`（workspace 共享根目录的 `target`）。

前端单独构建：

```powershell
cd app
pnpm build              # vue-tsc --noEmit && vite build
```

HEIC/AVIF、SVG 与 JPEG XL 解码为可选特性，默认开启，且均为纯 Rust 实现，不需要额外的 C 运行库或 DLL：

```powershell
cargo build --release --no-default-features                     # 不含 HEIC/AVIF/SVG/JXL
cargo build --release --no-default-features --features avif     # 仅 AVIF
cargo build --release --no-default-features --features heic     # 仅 HEIC/HEIF
cargo build --release --no-default-features --features svg      # 仅 SVG（resvg 光栅化）
cargo build --release --no-default-features --features jxl      # 仅 JPEG XL（jxl-oxide）
```

未包含对应解码器的构建打开这类文件时，会显示“解码器未包含在此构建中”的明确提示，不会崩溃。

命令行参数：

```sh
image-view [PATH]

  PATH            要打开的图片文件，或目录（打开其中第一张）
  -h, --help      显示帮助
  -V, --version   显示版本
```

日志默认写入安装目录的 `logs\image-view.log`（Windows 按用户安装即 `%LOCALAPPDATA%\image-view\logs\image-view.log`；目录不可写时回退到用户数据目录，macOS 固定为 `~/Library/Logs/image-view`；可用 `IMAGE_VIEW_LOG_DIR` 覆盖），同时输出到 stderr；每行格式为 `[<时间> <级别> <target>] <消息>`，时间按**本地时区**渲染并带 UTC 偏移（如 `2026-11-16T14:33:37.123+08:00`）；单文件超过 4 MiB 自动轮转，保留 3 份历史。可通过环境变量 `RUST_LOG` 控制级别；`RUST_LOG=image_view=debug` 可查看预加载、缓存命中等细节。崩溃时会在同目录生成 `crash-*.log`（含本地时间、backtrace，最多 10 份），可用“帮助 → 打开日志文件夹”直接定位。

## 隐私

应用不包含遥测、统计或自动联网上报；日志与崩溃转储只写入本机安装目录的 `logs\`（目录不可写时回退到用户数据目录）。自动更新默认不在启动时检查；只有点击「帮助 → 检查更新…」或勾选“启动时检查更新”后，才会访问 GitHub Releases 读取 `update.json`，且不会发送任何使用数据。

## 快捷键

| 按键 | 功能 |
| --- | --- |
| `Ctrl+O` | 打开文件 |
| `←` / `→` | 上一张 / 下一张（缩略图条同步居中） |
| `Ctrl+Shift+C` | 复制当前图片路径 |
| `Delete` | 移到回收站 |
| `Space` | 播放 / 暂停动画；当前图片不是动图时开始 / 停止幻灯片（3 秒间隔） |
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

PNG、JPEG、GIF、WebP、BMP、TIFF、ICO、QOI、TGA、PNM、HDR、EXR、DDS、farbfeld（`.ff`）、JPEG XL、AVIF、HEIC/HEIF、SVG。

APNG、GIF 与 WebP 支持多帧动画播放（含 alpha 与循环次数）；其余格式按静态图显示。

farbfeld 是 16-bit RGBA 无损格式，由 `image` 的通用解码路径按内容识别（不依赖扩展名），位深标记与 ICC 处理和其他 16-bit 图一致（该格式本身不带 ICC）。

HEIC/AVIF 默认启用，使用纯 Rust 解码器（`imgalifier-heif` / `imgalifier-avif`），不依赖 libheif 或系统 DLL。`.heif` 扩展名按 HEIC 处理。

JPEG XL 默认启用，使用纯 Rust 的 `jxl-oxide`（附 `image` 集成与 `lcms2` 特性），支持 8/16-bit 与浮点源图、alpha 与内嵌 ICC；CMYK 文件由解码器经 LCMS 转为 sRGB。

SVG 默认启用，使用 `resvg`（usvg + tiny-skia）光栅化，同样为纯 Rust、无系统 DLL：先解析出 `width`/`height`（缺失时回退 `viewBox`，两者都没有时按规范用 300×150）并做 250 MP 原图上限检查，再直接渲染到目标尺寸——矢量缩放即渲染，不会先画成位图再重采样。`.svgz`（gzip 压缩）暂不支持。

JPEG/TIFF/PNG/WebP/JXL/HEIC/AVIF 的内嵌 ICC profile 会转换到显示目标空间；无 profile 时按 sRGB 处理。CMYK 专用 profile 不受支持（解码后已转为 RGB），会回退原像素显示。

目标空间只有两档，因为画布能声明的空间只有两个（`CanvasRenderingContext2DSettings.colorSpace`）：`(color-gamut: p3)` 与 `getContext("2d", { colorSpace: "display-p3" })` 都成立时用 Display P3，否则 sRGB。预览 PNG 会写入对应的 `iCCP` 块——画布空间与标签不一致时浏览器会自行换算，所以即使在 P3 途中切回普通屏，颜色也不会错。

## 已知限制

- EXIF 方向校正只覆盖 JPEG/TIFF/WebP；HEIC/AVIF 使用容器内的旋转/镜像属性（`irot` / `imir`）。
- JPEG XL 只解静态图：`jxl-oxide` 的 `image` 集成尚未实现动画解码，`.jxl` 动画按首帧显示；浮点源图（HDR）按 8-bit 显示，不做色调映射。
- APNG 只支持 8-bit：16-bit APNG 的逐帧合成不在 `image` 解码器覆盖范围内，动画解码会失败，此时按 16-bit 静态首帧显示，并在顶部提示横幅说明原因。
- HEIC/AVIF 若只带 nclx 色彩原色（如未内嵌 ICC 的 Display P3），暂不进行色域转换；内嵌 ICC 会经 LCMS 转换到 sRGB。iPhone 照片通常内嵌 ICC。
- HEIC/AVIF 的 10/12-bit 源图由解码器内部转换为 8-bit 纹理；状态栏位深标记只区分通用解码路径的 8/16-bit。
- HEIC（HEVC/H.265）涉及专利许可，发布或商业分发前需自行评估；AVIF（AV1）不在此列。
- SVG 只渲染静态内容：不支持动画、脚本与事件；文字使用系统字体，缺少指定字体时回退到可用字体，嵌入式 Web 字体不加载。
- SVG 按声明的固有尺寸（`width`/`height`/`viewBox`，逻辑像素）做上限检查，超过 250 MP 会被拒绝打开；`.svgz`（gzip 压缩）不在支持列表内。
- 旋转仅作用于当前查看，不写回文件，切换图片后重置。
- 动图的帧间隔被钳制到 50~1000 ms（0 或超大 delay 都按此范围播放），因此极快或极慢的自定义时间轴不会与原始文件完全一致。
- 动图的总帧预算为 6400 万像素 / 1200 帧：超出上限的文件按静态首帧显示，不做逐帧播放，并在顶部提示横幅说明原因。动画帧解码失败时同样会提示。
- 动图只播放一次解码出的循环次数（取自文件元数据）；动画帧 PNG 逐帧生成，首次播放长动画的前几秒可能有轻微编码开销。
- 超大图会被降采样后再显示，此时「100%」是相对降采样后的纹理，状态栏会同时标注原图尺寸。
- 目录扫描不递归，且在打开文件时一次性完成；运行期间新增的文件需重新打开目录才可见。
- 界面偏好（语言、主题、缩略图条、色彩转换标记、启动时检查更新）、会话（上次目录/图片、排序）与窗口几何（大小/位置/最大化）都写在 `settings.ron`（Windows：`%APPDATA%\dev.jikun.imageview\settings.ron`），400 ms 防抖并在关闭窗口前同步落盘；旧版本单独的 `.window-state.json` 只在首次启动时迁移一次，之后不再写入。1.4.x 及更早存在 WebView 本地存储里的主题/语言/启动检查会在首次启动时自动转存一次，此后本地存储只作首帧缓存（免得启动瞬间闪一下默认配色），不再是持久化位置。
- 排序按修改时间读取文件元数据；网络盘或权限受限时，无法读取时间的文件排在最后。
- 删除只支持系统回收站；回收站不可用（如部分网络盘）时会提示错误并保留原文件。
- 主图纹理缓存最多保留 3 张图片（总像素 ≤ 72M，约 288 MB）；缩略图另有 512 张 LRU，与主图预算分离。
- 缩略图最长边为 128 px，适合 2× DPI 与常规缩放；极大目录下缩略图条只绘制视口附近卡片，但目录扫描本身仍是一次性完成。
- mipmap 会增加约 33% 纹理显存；当前主图与缩略图都启用 mipmap + LINEAR。
- 幻灯片到最后一张停止，不循环；手动翻页会重置 3 秒计时。
- 预加载会跳过超过 30 MP 的图片（仅影响速度，不影响正常打开）。
- 16-bit 图片在解码、ICC 转换与 PNG 传输中全程保留 16-bit 精度；显示端由 WebView 解码 PNG 后落到 8-bit Canvas，因此屏幕观感仍是 8-bit。10-bit/HDR 输出不在当前版本范围内。
- ICC 转换不支持 CMYK 专用 profile；profile 损坏或与图像通道不匹配时按原像素显示，具体原因写入 debug 日志。
- Display P3 需要广色域屏与支持该画布空间的引擎：WebView2 与新版本 WKWebView 可用，WebKitGTK 通常回退到 sRGB（颜色仍正确，只是吃不到广色域）。画布空间在首次绘制时固定，换显示器后需要重启才真正生效。
- 安装包与 exe 尚未签名，SmartScreen 可能提示“未知发布者”；首版接受该提示，后续版本再补代码签名。
- Windows 10/11 的默认应用由系统 `UserChoice` 保护：安装器写入扩展名关联后，若用户此前已在系统中选定其它默认应用，系统仍以 `UserChoice` 为准；请在「设置 → 应用 → 默认应用」中确认。
- 文件关联分两层写入：打包声明（`bundle.fileAssociations`）写 ProgID 与扩展名默认值，`app/src-tauri/installer/hooks.nsh` 写 `OpenWithProgids`、`Applications\<exe>\SupportedTypes` 与 `RegisteredApplications` + `Capabilities` 能力项。前者让双击可用，后者保证本程序始终出现在右键「打开方式」与「设置 → 默认应用」列表中；默认处理器本身仍由系统 `UserChoice` 决定，安装器不会强行改写。
- 安装向导支持英文 / 简体中文；应用内界面也可在“跟随系统 / 简体中文 / English”之间切换。自动更新只做 SHA-256 校验、没有代码签名；网络受限或需要回滚时，请从发布页手动下载安装包覆盖安装。
- 日志与崩溃转储写入安装目录的 `logs\`（Windows 即 `%LOCALAPPDATA%\image-view\logs`），卸载时随安装目录一并删除。若需长期保留问题现场，请在卸载前复制该目录。

## 项目结构

```plain
Cargo.toml          cargo workspace：成员、共享依赖、release profile 与版本号
app/
  src/              Vue 3 + TypeScript + Pinia 前端
  src-tauri/        Rust 适配层：命令、事件、iviewer:// 资源协议、原生菜单、自动更新
crates/             与 GUI 无关的业务逻辑，按依赖方向分层
  iv-domain/        排序、语言、主题与支持的扩展名
  iv-files/         目录扫描与自然/时间排序
  iv-application/   目录导航状态（上一张/下一张/选中）
  iv-metadata/      ICC profile 解析、sRGB 转换与 16-bit 精度
  iv-image-decode/  后台解码、优先级队列、预加载、降采样与缩略图
  iv-image-render/  缩放/平移视图数学（不依赖 GUI）
  iv-cache/         通用 LRU 与像素预算缓存
  iv-task/          后台请求代号（generation）
  iv-platform/      回收站删除、资源管理器定位与打开目录
  iv-infra/         日志轮转、设置持久化与存储端口
  iv-gui-contract/  GUI 动作与工具栏状态
  iv-library/       图库能力占位（尚未实现）
  iv-image-process/ 图片处理能力占位（尚未实现）
.github/workflows/  check.yml 日常检查、release.yml 推标签时三平台构建发布
docs/               路线图、分阶段计划与架构说明
design/             三端 UI 设计规范
CHANGELOG.md        各版本的用户可见变更
```

只列到目录一级。根目录另有 `rust-toolchain.toml`（工具链钉版）、`deny.toml`（cargo-deny 依赖审计）与自动生成的 `THIRD-PARTY-LICENSES.md`。各模块职责与依赖方向见 [`docs/03-tauri-vue-rust-architecture.md`](docs/03-tauri-vue-rust-architecture.md)，壳层与前端的约定见 [`AGENTS.md`](AGENTS.md)。

## 开发

```powershell
cargo fmt --all
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings

# 前端
cd app
pnpm build

# Tauri 开发与构建
cd app
pnpm tauri dev
pnpm tauri build
```

动画验收夹具用 `cargo run -p iv-image-decode --example animation_fixtures -- target/animation-fixtures` 生成：640×360 / 12 帧 / 80 ms 的常规 GIF、1500 ms 帧间隔且循环 2 次的 GIF、0 ms 帧间隔的 GIF，以及 4 帧无限循环 WebP，覆盖延迟钳制与循环次数两个边界。1.2.0 重写前的大图、色彩与缩略图夹具生成器（`gen_fixtures` / `color_fixtures` / `thumb_fixtures`）已随 eframe 版移除，需要时自行准备素材。

- 设计细节与取舍见 `docs/PLAN-P0.md`。
