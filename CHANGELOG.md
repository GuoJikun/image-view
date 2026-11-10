# 变更记录

本文件记录用户可见的变更，结构参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，版本号遵循[语义化版本](https://semver.org/lang/zh-CN/)。

## 版本策略

- 发布版本格式为 `MAJOR.MINOR.PATCH`；预发布追加 `-alpha.N`、`-beta.N` 或 `-rc.N`。
- `v2.0.0` 是 P1（M1~M6）完成后的首个正式版本；开发快照使用 `2.0.0-alpha.N`。
- 每个出版本在 git 打标签 `vX.Y.Z`，并在本文件新增对应章节；安装包文件名与 Add/Remove Programs 中的版本取自 `Cargo.toml`。
- Windows 文件版本使用去掉预发布后缀的四段数字（例如 `2.0.0-alpha.1` → `2.0.0.0`）。
- 不兼容的设置或缓存格式变更必须递增 `MAJOR`，或在章节中写明迁移/回退方式。

## [2.0.0-alpha.1] - 未发布

P1 开发快照：查看能力（M1~M4）与发布工程（M5）已完成，图库/搜索/对比（M6）尚未开始。

### 新增

- **安装与卸载**：Inno Setup 6 安装包，按用户安装到 `%LOCALAPPDATA%\Programs\image-view`，无需管理员权限；提供开始菜单快捷方式与可选桌面图标。
- **文件关联**：注册 PNG、JPEG、GIF、WebP、BMP、TIFF、ICO、QOI、TGA、PNM、HDR、EXR、DDS、AVIF、HEIC/HEIF 的 ProgID、`OpenWithProgids`、`Applications` 与 Windows“默认应用”能力项；卸载时清理，不残留注册表项。
- **崩溃日志**：日志写入 `%LOCALAPPDATA%\image-view\logs`，单文件 4 MiB 轮转、保留 3 份历史；panic 额外生成带 backtrace 的 `crash-*.log`（最多 10 份）；“文件 → 打开日志文件夹”可直接定位。
- **版本与发布**：引入本 CHANGELOG 与语义化版本规范；`scripts/build-release.ps1` 一键构建 release exe、安装包与 SHA256 校验文件。
- **应用图标**：生成多尺寸 `assets/icon.ico` 并链接进 exe，安装包、快捷方式与资源管理器使用同一图标。

### 变更

- 版本号从 `0.1.0` 提升为 `2.0.0-alpha.1`，对应 P1 开发线。
- release 构建保留符号表（`strip = "debuginfo"`），使崩溃 backtrace 仍包含函数名；安装包体积相应略有增加。
- 日志默认同时写入文件和 stderr；release 版本没有控制台时以文件为准。

### 已知限制

- 安装包未做代码签名，Windows SmartScreen 可能提示“未知发布者”；首版接受该提示并在 README 说明。
- Windows 10/11 不允许程序静默修改默认看图程序。安装器注册“打开方式”和“默认应用”能力项，并提供打开系统设置的入口；用户仍需在“设置 → 应用 → 默认应用”中确认。
- 自动更新与中/英 UI 资源表按计划推迟到 v2.1 / v2.2，本版本不包含更新检查。
- 不包含遥测、统计或自动联网上报；日志与崩溃转储仅保存在本机。
- 未在干净 Win10/11 虚拟机上完成安装、升级、卸载的完整验收；本地已完成安装/静默卸载与注册表清理验证。

## [0.1.0] - 2026-11-10

初始开发版本（v1.0 / v1.1 功能集）：

- 文件对话框、命令行、拖放打开；目录自然排序与上一张/下一张。
- 适配窗口、100%、1%~3200% 缩放、平移、双击切换、全屏。
- EXIF 方向校正与 90° 旋转（不写回文件）。
- 异步单线程解码、latest-wins、加载中提示、错误横幅与重试。
- 大图保护：单边 8192、40 MP 纹理上限、250 MP 原图拒绝、自动降采样。
- 幻灯片、相邻图片预加载、LRU 纹理缓存、回收站删除、复制路径、资源管理器定位。
- 窗口位置/大小/最大化、上次目录/图片与排序方式持久化。
- ICC → sRGB 色彩管理与 16-bit 精度保持。
- HEIC/AVIF 纯 Rust 解码（默认开启，可裁剪）。
- 缩略图条、虚拟滚动、缩略图内存 LRU 与磁盘缓存、mipmap + glow 渲染。
