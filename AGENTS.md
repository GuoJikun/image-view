# AGENTS.md

面向在本仓库工作的 AI 编码工具。工具无关的全局偏好（中文交流、Angular 提交）见 `~/AGENTS.md`，
本文件只写 image-view 仓库特有的约定；设计与验收细节以 `docs/` 下的文档为准。

## 项目速览

- 桌面看图应用：Rust workspace 承载业务逻辑，`app/` 是 Tauri 2 壳 + Vue 3 / Vite / Pinia 前端。
- 三端发布（Windows / macOS / Linux），Windows 是主要开发与验证平台。
- 入口文档：[README.md](README.md)（用户视角）、[docs/ROADMAP.md](docs/ROADMAP.md)（进度）、
  `docs/PLAN-P0.md` ~ `docs/PLAN-P3.md`（分阶段设计与验收记录）。

## 质量门禁

合并前四条必须全过。不要为了过检查去改断言、放宽阈值或 `--no-verify` 跳钩子：

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm --dir app build
```

本地开发：`cd app && pnpm tauri dev`；改完前端跑一次 `pnpm --dir app build`（含 `vue-tsc`）。

## 代码分层

- 业务逻辑放 `crates/iv-*`，**不要写进 `app/src-tauri`**；壳层只做参数转换、错误码映射与事件派发。
  `iv-domain` 领域类型 · `iv-files` 目录扫描排序 · `iv-application` 导航状态 · `iv-metadata` ICC
  · `iv-image-decode` 解码/预加载/缩略图 · `iv-image-render` 缩放平移数学 · `iv-cache` LRU 与像素预算
  · `iv-task` 请求代号 · `iv-platform` 回收站与资源管理器 · `iv-infra` 日志与设置持久化。
- 前端访问 Tauri 的唯一出口是 `app/src/services/ipc.ts`；组件与 store 不直接 `invoke`。
- 新增命令或事件要成对改：`app/src-tauri/src/dto.rs`（camelCase 序列化）+
  `app/src/types/ipc.ts`（类型）；前端注册监听走 `ipc.ts` 的封装。
- 状态放 `app/src/stores/`（Pinia），组件保持无状态展示。

## 资源协议

- 图片资源走自定义协议：其他平台是 `iviewer://localhost/…`，Windows 上是
  `http://iviewer.localhost/…`（前端由 `app/src/services/ipc.ts` 的 `resourceUrl` 统一分派，别自己拼）。
- 只接受 UUID 授权句柄（`imageId` + `revision`），**禁止用文件路径拼 URL**。
- 现有资源：`preview`（当前图 PNG）、`thumbnail/{id}/{size}/{rev}`、`frame/{id}/{index}/{rev}`（动画帧）。
  响应按不可变资源长缓存，文件变了靠 `revision` 换 URL 才回源。

## 前端规范

- 组件**不写** `<style>` / `<style scoped>`，样式全部集中在 `app/src/styles/main.css`，
  按 `/* ---------- 区块 ---------- */` 分段，并遵守文件内既有的书写顺序。
- 颜色与尺寸走设计令牌。新增或改颜色必须同步**四处**：`:root`、
  `@media (prefers-color-scheme: dark)`、`:root[data-theme="light"]`、`:root[data-theme="dark"]`。
- 界面文案必须中英同步改 `app/src/i18n.ts`，不要把中文硬编码进模板。
- 缩略图条布局常量：卡片 76×56、`ITEM_WIDTH = 85`（含 9px 间距）、`PAD = 16`、条带高 88px。
  横向滚动用自绘 `AppScrollbar.vue`，不要回退到 `scrollbar-width` / `::-webkit-scrollbar` 美化
  —— 原生滚动条会吃掉 88px 条带高度。
- 视觉与尺寸依据 `design/macos-first/README.md`。

## 发布（版本号 X.Y.Z）

1. 改 4 处版本号：`Cargo.toml` 的 `[workspace.package]`、`Cargo.lock`（跑一次 `cargo metadata` 自动同步
   `image-view` 与各 `iv-*` 条目）、`app/src-tauri/tauri.conf.json`、`app/package.json`。
   `app/src-tauri/Cargo.toml` 继承 workspace，无需改；前端不存版本号常量
   （更新弹窗用 `@tauri-apps/api/app` 的 `getVersion()` 运行时读取）。
2. `CHANGELOG.md`：把 `## [未发布]` 改名为 `## [X.Y.Z] - 日期`，顶部再留一个空的 `## [未发布]`；
   按「新增 / 变更 / 修复」补齐未推送提交对应的条目。
3. 同步文档：`README.md`（当前版本段、tag 示例）、`docs/ROADMAP.md`（当前版本、下一里程碑、最近完成表）。
4. 提交 `chore(release): 版本号调整为 X.Y.Z`，打**附注标签**
   `git tag -a vX.Y.Z -m "image-view X.Y.Z"`，再推 main 与 tag。
5. Release 正文由 workflow 从 CHANGELOG 对应版本段抽取，段落缺失会让 validate 阶段失败。
   `tauri-action` 只在创建 Release 时写正文，重跑同一 tag 不会刷新。

## 文档措辞

- Tauri 是唯一实现：**不写「Tauri 版」这类实现归属标注**（里程碑表、验收记录标题、变更记录、
  代码注释都一样）。
- 需要交代历史时用版本或时间：「1.2.0 起」「1.2.0 之前的 eframe 版」
  （统一写 `eframe 版`，不写 `egui 版`）；重写记录标题格式为「P1-Mx 重写：…（1.2.0，日期）」。
- 验收记录写清环境、方法、实测数据与**未覆盖项**；没做目视确认就如实写没做。

## 测试与夹具

- 单测跟实现放同一文件；涉及磁盘的用 `std::env::temp_dir()` 下的独立目录，用完清理。
- 动画夹具：`cargo run -p iv-image-decode --example animation_fixtures`，产物在 `target/animation-fixtures/`。
  （1.2.0 重写前的 `gen_fixtures` / `color_fixtures` / `thumb_fixtures` 随 eframe 版一并移除，命令已不可用。）
- 依赖审计：`cargo deny check`（配置见 `deny.toml`，离线时加 `--offline`）。
