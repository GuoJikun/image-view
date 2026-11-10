# 图片查看器（image-view）

一个轻量的 Rust 桌面图片查看器，基于 **eframe/egui 0.37**。目标是把「打开图片、看清细节」这件事做快、做稳。

## 功能

- 通过文件对话框（Ctrl+O）、命令行参数或拖拽打开图片
- 「适配窗口」与「100%」（1 图像像素 = 1 物理屏幕像素）
- 以光标为锚点的滚轮 / 触控板缩放，鼠标拖拽平移，双击在「适配」与「100%」之间切换
- 同一目录内上一张 / 下一张（按文件名自然排序，不循环）
- 状态栏：文件名、原图尺寸/显示尺寸、缩放百分比、序号、加载与降采样提示
- 后台线程解码，快速翻页时只保留最新请求，旧结果自动丢弃
- 大图安全阈值：单边 8192、总像素 40MP 自动等比降采样；超过 250MP 的原图直接拒绝解码
- 中英文界面（默认深色主题，中文界面）

## 运行

需要 Rust 1.95+（edition 2024）。

```powershell
cargo run --release
cargo run --release -- "D:\photos\a.jpg"
cargo run --release -- "D:\photos"      # 打开目录中第一张图片
```

构建产物位于 `target\release\image-view.exe`，可自行复制到任意位置使用。

命令行参数：

```
image-view [PATH]

  PATH            要打开的图片文件，或目录（打开其中第一张）
  -h, --help      显示帮助
  -V, --version   显示版本
```

日志可通过环境变量 `RUST_LOG` 控制，例如 `RUST_LOG=debug`。

## 快捷键

| 按键 | 功能 |
| --- | --- |
| `Ctrl+O` | 打开文件 |
| `←` / `→` | 上一张 / 下一张 |
| `+` / `-`（或滚轮） | 放大 / 缩小 |
| `0` | 适配窗口 |
| `1` | 100% |
| `F` | 在「适配」与「100%」之间切换 |
| 双击画布 | 同上 |
| `F11` | 全屏 |
| `Esc` | 退出全屏 / 关闭帮助窗口 |

## 支持的格式

PNG、JPEG、GIF、WebP、BMP、TIFF、ICO、QOI、TGA、PNM、HDR、EXR、DDS。
（未启用 AVIF，以控制依赖体积与编译时间。）

## 已知限制

- 暂不处理 EXIF 方向，部分手机照片可能方向不正确（P1）。
- 动图只显示第一帧。
- 超大图会被降采样后再显示，此时「100%」是相对降采样后的纹理，状态栏会同时标注原图尺寸。
- 目录扫描不递归，且在打开文件时一次性完成；运行期间新增的文件需重新打开目录才可见。

## 项目结构

```
src/
  main.rs       入口：CLI、窗口参数、图标
  app.rs        应用状态与每帧编排（eframe::App 的 logic / ui）
  ui.rs         顶部工具栏、底部状态栏、错误横幅、帮助窗口
  viewer.rs     缩放/平移数学与画布绘制
  document.rs   当前图片与 GPU 纹理
  loader.rs     后台解码线程、阈值检查、降采样
  folder.rs     目录扫描、自然排序、上一张/下一张
assets/
  icon.png      窗口图标
docs/
  PLAN.md       设计与实现方案
```

## 开发

```powershell
cargo fmt
cargo test
cargo clippy --all-targets -- -D warnings
cargo build --release
```

- 设计细节与取舍见 `docs/PLAN.md`。
- 渲染后端默认使用 wgpu；若在老显卡驱动上遇到问题，可启用 eframe 的 `glow` 特性并切换 `NativeOptions.renderer`。
