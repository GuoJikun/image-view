# image-view 路线图与进度总览（Iced GUI 分支）

| 项目 | 内容 |
| --- | --- |
| 最后更新 | 2026-11-17 |
| 分支 | `gui-iced`：Iced GUI 分支，不合并回 `main`，只从 `main` 合并 `crates/` 与工作区根 `Cargo.toml` |
| 当前版本 | v1.5.0（版本号继承自工作区，随 `main` 同步；本分支不产出安装包） |
| GUI 现状 | 纯 Rust Iced 0.14（`app`），功能范围见 [02-iced-architecture.md](02-iced-architecture.md) §12 |
| 明细计划 | [PLAN-P0.md](PLAN-P0.md) · [PLAN-P1.md](PLAN-P1.md) · [PLAN-P2.md](PLAN-P2.md) · [PLAN-P3.md](PLAN-P3.md) |

> 本文件只做**汇总与进度标记**。需求细节、验收标准与设计取舍写在各自的阶段计划里；两者冲突时以阶段计划为准。
> 阶段里程碑描述的是**共享 crates（从 `main` 合并）**的完成情况。GUI 相关能力以本分支实际代码为准，
> 阶段计划里出现的 Tauri / Vue / WebView 条目属于主分支形态，不代表本分支已实现。

## 阶段进度（共享 crates）

| 阶段 | 版本 | 主题 | 状态 | 明细 |
| --- | --- | --- | --- | --- |
| P0 | 1.0.0 开发基线 | 查看器基线 | ✅ 已完成 | [PLAN-P0.md](PLAN-P0.md) |
| P1 | v1.0 | 格式、色彩、浏览与产品化 | ✅ 已完成 | [PLAN-P1.md](PLAN-P1.md) |
| P2 | v2.x | 专业能力、编辑与架构增强 | ⬜ 未开始 | [PLAN-P2.md](PLAN-P2.md) |
| P3 | v3+ | 长期演进（新增） | ⬜ 未排期 | [PLAN-P3.md](PLAN-P3.md) |

图例：✅ 已完成 · 🚧 进行中 · ⬜ 未开始 / 未排期 · ⏸ 暂停 · ❌ 取消

P0 的里程碑为 [PLAN-P0.md](PLAN-P0.md) §8 的 M0 ~ M7；P1 及之后统一使用 `P<n>-M<n>` 编号，P3 为方向性条目，不计入完成度。

## Iced GUI 现状

本分支的 GUI 在 `app`，与 `main` 的 Tauri GUI 是两套独立实现。已实现与未完成清单以
[02-iced-architecture.md](02-iced-architecture.md) §12.2 / §12.3 为准，摘要：

- **已实现**：命令行 / 对话框 / 拖放打开、单层目录翻页与排序、缩放平移与 90° 旋转、适配 / 100%、
  虚拟滚动缩略图条、元数据面板、3 秒幻灯片、回收站删除 / 复制路径 / 资源管理器定位、全屏、
  `settings.ron` 会话与偏好持久化；界面已按 `design/macos-first` 规范做视觉规范化（色板令牌、
  图标工具栏、缩放胶囊、空状态与错误横幅、Windows / Linux 自绘标题栏）；
  窗口内菜单栏（文件 / 显示 / 帮助，含勾选项、子菜单与平台快捷键）、中英双语界面与语言切换、
  菜单里的主题切换与快捷键弹窗。
- **未完成**：系统原生菜单栏、动画逐帧播放、文件监听、单实例 / 文件关联、触控板手势；
  这些能力在主分支存在与否都不构成本分支的验收项。

## 当前焦点

- **同步主线**：定期把 `main` 的 `crates/` 与工作区根 `Cargo.toml` 合并进来，保持 Core 能力与主线一致，
  再按需在 Iced GUI 接线。
- **质量门禁**：`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、
  `cargo test --workspace` 全部通过；GUI 冒烟测试能正常打开图片。
- **完成定义**：新增功能先在 Core（`crates/iv-*`）补用例，GUI 只做映射；单测跟实现放同一文件；
  验收记录写清环境、方法、实测数据与未覆盖项。

## P1 里程碑看板（v1.0，共享 crates）

| ID | 里程碑 | 状态 | 完成日期 |
| --- | --- | --- | --- |
| P1-M1 | 桌面集成与持久化 | ✅ | 2026-11-10 |
| P1-M2 | ICC 色彩管理与 16-bit | ✅ | 2026-11-10 |
| P1-M3 | HEIC / AVIF | ✅ | 2026-11-10 |
| P1-M4 | 缩略图条与渲染质量 | ✅ | 2026-11-10 |
| P1-M5 | 安装、关联与发布工程 | ✅ | 2026-11-10 |
| P1-M8 | 自动更新（检查/下载/校验/安装） | ✅ | 2026-11-12 |
| P1-M9 | 中/英多语言与语言设置 | ✅ | 2026-11-12 |
| P1-M6 | 图库、搜索与对比（只读） | ⏸ 延后至 v1.1 | — |
| P1-M7 | GIF / WebP 动画 | ✅ | 2026-11-16 |

> M5 / M8 / M9 的「安装、关联、自动更新、多语言」是 `main` 上 Tauri GUI 的交付物；其 Core 部分
> 随 `crates/` 合并进本分支，GUI 表现请以「Iced GUI 现状」一节为准。

## P2 / P3 摘要

| 阶段 | 方向 | 状态 | 明细 |
| --- | --- | --- | --- |
| P2 | RAW、HDR/10-bit 完整链路、分块流式加载与 GPU 解码、插件、设置与主题、多窗口、超大目录架构、基础编辑 | ⬜ 未开始 | [PLAN-P2.md](PLAN-P2.md) |
| P3 | 云同步与分享、AI 辅助、移动端/Web、插件市场、协作标注、企业部署（均新增、未排期） | ⬜ 未排期 | [PLAN-P3.md](PLAN-P3.md) |

## 最近完成

| 日期 | 内容 | 提交 |
| --- | --- | --- |
| 2026-11-17 | Iced GUI 按 [design/macos-first](../design/macos-first/README.md) 完成界面视觉规范化：令牌化浅/深色板、图标工具栏与缩放胶囊、缩略图卡片、状态栏、空状态与错误横幅、拖放提示，Windows / Linux 自绘标题栏 | `dd693d3` |
| 2026-11-17 | 目录结构整理：`apps/iced-app` 提升为 `app`；workspace 依赖只保留被 ≥2 个成员共享的 6 条，其余内联或删除 | `277ee2e` |
| 2026-11-17 | 合并 `main` 的非 GUI 部分到本分支：`crates/*` 与工作区 `Cargo.toml`（farbfeld / JPEG XL / APNG、源位深保留、广色域转换、动画降级提示、磁盘缩略图缓存与日志等），并按实际代码重写核心文档 | `456a7bc` |
| 2026-11-16 | v1.5.0：GIF/WebP 动画逐帧播放、界面偏好统一进 `settings.ron`、macOS 原生菜单勾选状态（`main` 交付，Core 部分随合并进入本分支） | `e0c437a` 等 |
| 2026-11-14 | v1.2.0 ~ v1.4.1：Tauri + Vue 迁移、三端打包与自动更新、窗口/会话记忆与缩略图磁盘缓存（`main` 交付） | 见 CHANGELOG.md |
| 2026-11-10 | P1-M1 ~ M5 与 P0 基线落地；Iced GUI 采用同一套 Core crate | 见 PLAN-P0/P1.md |

## 进度维护规则

1. **完成子项**：先在对应阶段计划勾选条目，再更新本文件的里程碑状态与「最近完成」。
2. **里程碑完成**：填写完成日期，更新「当前焦点」为下一项。
3. **同步主线**：合并 `main` 的 `crates/` / `Cargo.toml` 后，在「最近完成」记录合并进来的关键能力。
4. **需求变更**：新需求先归入 P1/P2/P3，不允许存在「无阶段需求」；跨阶段移动或取消时在目标阶段留痕。
5. **暂停/取消**：状态改为 ⏸ / ❌，在里程碑表备注原因，不直接删除条目。
6. **P0**：已完成并冻结范围；新需求一律进入 P1 或之后。

## 质量门禁

改动合入前必须通过：

```text
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

本地运行：`cargo run -p image-view -- <路径>`。CI 配置由维护者单独维护，本文档不描述其细节。

## 文档索引

| 文档 | 用途 |
| --- | --- |
| [README.md](../README.md) | 本分支使用说明、快捷键、支持格式、已知限制 |
| [02-iced-architecture.md](02-iced-architecture.md) | Iced + Rust 架构与本分支实施记录（现行） |
| [01-eframe-egui-architecture.md](01-eframe-egui-architecture.md) | eframe / egui 方案设计档案 |
| [03-tauri-vue-rust-architecture.md](03-tauri-vue-rust-architecture.md) | Tauri + Vue 方案设计档案（`main` 形态） |
| [TAURI-POC.md](TAURI-POC.md) | Tauri POC 记录（历史档案） |
| [PLAN-P0.md](PLAN-P0.md) | P0（1.0.0 开发基线）设计与实现细节、验收记录、性能预算 |
| [PLAN-P1.md](PLAN-P1.md) | P1（v1.0）：M1~M9 里程碑、验收与发布门槛 |
| [PLAN-P2.md](PLAN-P2.md) | P2（v2.x）：专业能力、编辑（M8）与架构增强 |
| [PLAN-P3.md](PLAN-P3.md) | P3（v3+）：长期方向（探索性、未排期） |
| [ROADMAP.md](ROADMAP.md) | 本文件：阶段状态、里程碑看板与更新规则 |
