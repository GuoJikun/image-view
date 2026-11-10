# 图片查看器（image-view）

一个轻量的 Rust 桌面图片查看器，基于 **eframe/egui 0.37**。目标是把「打开图片、看清细节」这件事做快、做稳。

## 功能

- 通过文件对话框（Ctrl+O）、命令行参数或拖拽打开图片
- 「适配窗口」与「100%」（1 图像像素 = 1 物理屏幕像素）
- 自动按 EXIF 方向校正（JPEG/TIFF/WebP），HEIC/AVIF 应用容器内的旋转/镜像属性；`R` / `Shift+R` 顺时针 / 逆时针旋转 90°
- 读取 JPEG/TIFF/PNG/WebP/HEIC/AVIF 内嵌 ICC profile 并转换到 sRGB；16-bit PNG/TIFF 在转换与缩放阶段保持精度，最终以 8-bit 纹理显示
- 以光标为锚点的滚轮 / 触控板缩放，鼠标拖拽平移，双击在「适配」与「100%」之间切换
- 同一目录内上一张 / 下一张（按文件名自然排序，不循环），可切换按修改时间排序
- 记住窗口位置/大小/最大化、上次目录与图片、排序方式；无参数启动时恢复上次会话
- `Delete` 删除到系统回收站，`Ctrl+Shift+C` 复制路径，“文件”菜单或右键菜单可在资源管理器中定位
- 幻灯片：按 `Space` 每 3 秒自动前进，到最后一张自动停止
- 相邻图片预加载 + 最近使用（LRU）纹理缓存，最多 3 张，回翻立即显示
- 状态栏：文件名、原图尺寸/显示尺寸与位深、缩放百分比、序号、加载与降采样提示；可在“文件”菜单开启色彩转换标记
- 后台单线程解码，打开请求优先于预加载；快速翻页时只保留最新请求，旧结果自动丢弃
- 大图安全阈值：单边 8192、总像素 40MP 自动等比降采样；超过 250MP 的原图直接拒绝解码；预加载只处理 30MP 以下的图片
- 中文界面（默认深色主题）

## 版本与路线

当前为 v1.1 功能集（P0 + EXIF/旋转/全屏 + 幻灯片/预加载缓存），并已完成 v2.0 的 P1-M1（桌面集成与持久化）、P1-M2（ICC 色彩管理与 16-bit）与 P1-M3（HEIC / AVIF）。默认构建包含 HEIC/AVIF 解码器，release exe 约 20.1 MB。

- 阶段划分与进度：[docs/ROADMAP.md](docs/ROADMAP.md)
- v2.0 计划：[docs/PLAN-P1.md](docs/PLAN-P1.md)
- v2.x / v3+ 方向：[docs/PLAN-P2.md](docs/PLAN-P2.md) · [docs/PLAN-P3.md](docs/PLAN-P3.md)
- v1.x 设计与验收：[docs/PLAN-P0.md](docs/PLAN-P0.md)

## 运行

需要 Rust 1.95+（edition 2024）。

```powershell
cargo run --release
cargo run --release -- "D:\photos\a.jpg"
cargo run --release -- "D:\photos"      # 打开目录中第一张图片
```

构建产物位于 `target\release\image-view.exe`，可自行复制到任意位置使用。

HEIC/AVIF 解码为可选特性，默认开启，且均为纯 Rust 实现，不需要额外的 C 运行库或 DLL：

```powershell
cargo build --release --no-default-features                     # 不含 HEIC/AVIF
cargo build --release --no-default-features --features avif     # 仅 AVIF
cargo build --release --no-default-features --features heic     # 仅 HEIC/HEIF
```

未包含对应解码器的构建打开这类文件时，会显示“解码器未包含在此构建中”的明确提示，不会崩溃。

命令行参数：

```
image-view [PATH]

  PATH            要打开的图片文件，或目录（打开其中第一张）
  -h, --help      显示帮助
  -V, --version   显示版本
```

日志可通过环境变量 `RUST_LOG` 控制；`RUST_LOG=image_view=debug` 可查看预加载、缓存命中等细节。

## 快捷键

| 按键 | 功能 |
| --- | --- |
| `Ctrl+O` | 打开文件 |
| `←` / `→` | 上一张 / 下一张 |
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

## 支持的格式

PNG、JPEG、GIF、WebP、BMP、TIFF、ICO、QOI、TGA、PNM、HDR、EXR、DDS、AVIF、HEIC/HEIF。

HEIC/AVIF 默认启用，使用纯 Rust 解码器（`imgalifier-heif` / `imgalifier-avif`），不依赖 libheif 或系统 DLL。`.heif` 扩展名按 HEIC 处理。

JPEG/TIFF/PNG/WebP/HEIC/AVIF 的内嵌 ICC profile 会转换到 sRGB；无 profile 时按 sRGB 处理。CMYK 专用 profile 不受支持（解码后已转为 RGB），会回退原像素显示。

## 已知限制

- EXIF 方向校正只覆盖 JPEG/TIFF/WebP；HEIC/AVIF 使用容器内的旋转/镜像属性（`irot` / `imir`）。
- HEIC/AVIF 若只带 nclx 色彩原色（如未内嵌 ICC 的 Display P3），暂不进行色域转换；内嵌 ICC 会经 LCMS 转换到 sRGB。iPhone 照片通常内嵌 ICC。
- HEIC/AVIF 的 10/12-bit 源图由解码器内部转换为 8-bit 纹理；状态栏位深标记只区分通用解码路径的 8/16-bit。
- HEIC（HEVC/H.265）涉及专利许可，发布或商业分发前需自行评估；AVIF（AV1）不在此列。
- 旋转仅作用于当前查看，不写回文件，切换图片后重置。
- 动图只显示第一帧。
- 超大图会被降采样后再显示，此时「100%」是相对降采样后的纹理，状态栏会同时标注原图尺寸。
- 目录扫描不递归，且在打开文件时一次性完成；运行期间新增的文件需重新打开目录才可见。
- 设置与窗口状态保存在 `%APPDATA%\image-view\data\app.ron`；文件损坏或版本过旧时自动回退默认值。
- 排序按修改时间读取文件元数据；网络盘或权限受限时，无法读取时间的文件排在最后。
- 删除只支持系统回收站；回收站不可用（如部分网络盘）时会提示错误并保留原文件。
- 纹理缓存最多保留 3 张图片（总像素 ≤ 72M，约 288 MB），内存占用会随打开的图片增加。
- 幻灯片到最后一张停止，不循环；手动翻页会重置 3 秒计时。
- 预加载会跳过超过 30 MP 的图片（仅影响速度，不影响正常打开）。
- 16-bit 图片以 8-bit 纹理显示（色彩转换阶段仍保留 16-bit 精度）；10-bit/HDR 输出不在当前版本范围内。
- ICC 转换不支持 CMYK 专用 profile；profile 损坏或与图像通道不匹配时按原像素显示，具体原因写入 debug 日志。

## 项目结构

```
src/
  main.rs       入口：CLI、窗口参数、图标
  app.rs        应用状态与每帧编排（eframe::App 的 logic / ui）
  ui.rs         顶部工具栏、底部状态栏、错误横幅、帮助窗口
  viewer.rs     缩放/平移数学与画布绘制
  document.rs   当前图片与 GPU 纹理
  cache.rs      最近使用（LRU）纹理缓存
  loader.rs     后台解码线程、优先级队列、预加载、阈值检查、降采样、HEIC/AVIF 分派
  color.rs      ICC profile 解析、sRGB 转换与 16-bit 精度保持
  folder.rs     目录扫描、自然排序/时间排序、上一张/下一张
  settings.rs   持久化设置：窗口状态、上次会话与排序方式
  fileops.rs    回收站删除与在资源管理器中定位
examples/
  gen_fixtures.rs    生成手工测试用的图片夹具（开发用）
  color_fixtures.rs  生成 AdobeRGB / Display P3 / sRGB 色彩验收夹具
assets/
  icon.png      窗口图标
docs/
  ROADMAP.md    阶段进度总览与里程碑
  PLAN-P0.md    v1.x 设计与验收记录
  PLAN-P1.md    v2.0 计划（M1~M7，只查看）
  PLAN-P2.md    v2.x 专业能力、编辑与架构
  PLAN-P3.md    v3+ 长期方向
```

## 开发

```powershell
cargo fmt
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

手工测试夹具可用 `cargo run --release --example gen_fixtures -- <输出目录>` 生成（会写入约 100MP 的 JPEG，注意磁盘空间）。色彩验收夹具用 `cargo run --release --example color_fixtures -- <输出目录>` 生成。

- 设计细节与取舍见 `docs/PLAN-P0.md`。
- 渲染后端默认使用 wgpu；若在老显卡驱动上遇到问题，可启用 eframe 的 `glow` 特性并切换 `NativeOptions.renderer`。
