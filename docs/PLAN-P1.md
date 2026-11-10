# P1 计划：v1.0 格式、色彩、浏览与产品化

| 项目 | 内容 |
| --- | --- |
| 阶段 | P1 |
| 版本 | v1.0（部分里程碑可拆到 v1.1 / v1.2） |
| 状态 | 🚧 进行中（M1~M5 已完成） |
| 估时 | 11~16 个工作日（单人） |
| 前置 | P0 已完成（见 [PLAN-P0.md](PLAN-P0.md)） |
| 总览 | [ROADMAP.md](ROADMAP.md) |
| 后续 | [PLAN-P2.md](PLAN-P2.md) · [PLAN-P3.md](PLAN-P3.md) |

> 本文只记录 P1 尚未实现的需求。P0 基线与验收记录见 [PLAN-P0.md](PLAN-P0.md)；P2 / P3 为方向性规划，见对应文件。
> 进度以复选框和 [ROADMAP.md](ROADMAP.md) 为准。

## 1. 目标与范围

**目标**：把 image-view 从“能看图”升级为“日常主力看图 + 轻量管理”，覆盖格式、色彩、连续浏览与可分发安装。

**优先级**：P1 只做**查看 + 发布**。M1~M4 围绕“看得更快、更准、更顺”，M5 完成发布工程，图库（M6）排在发布工程之后、但**仍属于 v1.0**；动画（M7）排在最后，作为 v1.1 候选。**编辑能力已移出 P1**，见 [PLAN-P2.md](PLAN-P2.md) 的 P2-M8。

**非目标**：RAW、AI、云同步、移动端（均属 P2 / P3）；图片编辑（属 P2，不在 v1.0 范围）。

**发布门槛**：P1-M1 ~ P1-M6 全部完成 → v1.0.0（执行顺序：M1~M4 查看 → M5 安装工程 → M6 图库）；P1-M7（GIF / WebP 动画）与 P1-M5 的自动更新/多语言可拆分为 v1.1 / v1.2。

## 2. 里程碑总览

| 里程碑 | 内容 | 估时 | 状态 |
| --- | --- | --- | --- |
| P1-M1 | 桌面集成与持久化 | 0.5~1 d | ✅ |
| P1-M2 | ICC 色彩管理与 16-bit | 1.5~2 d | ✅ |
| P1-M3 | HEIC / AVIF | 1~1.5 d | ✅ |
| P1-M4 | 缩略图条与渲染质量 | 2~3 d | ✅ |
| P1-M5 | 安装、关联、更新与多语言 | 2~3 d | ✅ |
| P1-M6 | 图库、搜索与对比（只读） | 3~4 d | ⬜ |
| P1-M7 | GIF / WebP 动画（最后做） | 1~1.5 d | ⬜ |

合计约 **11~16 个工作日**（单人，不含返工与测试夹具准备）。编辑功能不在 P1 内，已归入 [PLAN-P2.md](PLAN-P2.md) P2-M8；图库排在发布工程之后但仍在 v1.0 内，动画可延后到 v1.1。

## 3. 里程碑详情

### 3.1 P1-M1 桌面集成与持久化（0.5~1 d）

- [x] 记住窗口大小、位置、最大化状态（`eframe` persistence 或 `%APPDATA%\image-view\settings.json`）。
- [x] 记住上次目录：无命令行参数启动时恢复到上次浏览位置。
- [x] 记住当前目录内的浏览位置与排序方式。
- [x] 删除到回收站（`trash` crate；`Delete` 键 + 工具栏/右键菜单）。
- [x] 复制文件路径（`Ctrl+Shift+C`）与“在资源管理器中显示”。
- [x] 设置文件损坏、无权限或版本过旧时回退默认值，不阻塞启动。

**验收**

- 调整窗口 → 退出 → 重启，尺寸/位置/目录/图片序号全部恢复。
- 删除当前图片后自动前进到下一张；目录清空时回到空状态。
- 只读目录、ACL 拒绝场景不 panic、不产生错误横幅噪音。

**风险**：跨平台配置目录差异 → 复用 eframe `persistence` 的存储位置（Windows：`%APPDATA%\image-view\data`）；设置项增多时在 `settings.rs` 内按 schema 版本扩展。

### 3.2 P1-M2 色彩管理与高位深（1.5~2 d）

- [x] 读取 JPEG/TIFF/PNG/WebP 内嵌 ICC profile。
- [x] 用 `lcms2` 将源色彩空间转换到 sRGB 后再生成纹理。
- [x] 16-bit PNG/TIFF 在转换阶段保留色深，最终输出 8-bit 纹理（显示端仍为 8-bit）。
- [x] 无 ICC 时按 sRGB 处理；profile 解析/转换失败时回退原像素并写 debug 日志。
- [x] 状态栏可显示“已色彩转换”标记（可选，默认关闭）。

**验收**

- 带 AdobeRGB / Display P3 profile 的图片与系统查看器、浏览器颜色接近（同屏对比）。
- 16-bit 渐变图不出现明显色带；转换失败的图片仍可正常显示。

**风险**：`lcms2` 原生依赖与体积 → 实测 `lcms2-sys` 可用 MSVC 工具链静态编译，无需随包分发 DLL；release 体积增加约 1.1 MB，暂不引入 feature gate。

### 3.3 P1-M3 HEIC / AVIF（1~1.5 d）

- [x] AVIF 解码（`imgalifier-avif`，纯 Rust `rav1d` 后端），扩展名与文件过滤器同步。
- [x] HEIC 解码（`imgalifier-heif` 的纯 Rust HEVC 解码器，无需 libheif 或随包 DLL）。
- [x] 缺失解码器时给出明确错误文案，而不是“格式不受支持”泛化提示。
- [x] 评估安装包体积与许可；默认构建可关闭，安装器提供可选组件。

**验收**

- iPhone 导出的 HEIC 与 AVIF 样例正常显示、可旋转、可翻页。
- 无解码器的构建打开对应文件时提示清晰、不崩溃。

**风险**：C 依赖、许可与体积 → 实际采用纯 Rust 解码器，release 仅增加约 2.0 MB；`avif` / `heic` 为 Cargo 特性，默认开启、可单独关闭。HEVC（H.265）专利许可仍需在商业分发前自行评估。

### 3.4 P1-M4 浏览体验（2~3 d）

- [x] 缩略图条 / 胶片模式：当前目录、懒加载、虚拟滚动。
- [x] 缩略图磁盘缓存（`%LOCALAPPDATA%\image-view\thumbs`，键 = 路径 + 修改时间 + 目标尺寸）。
- [x] 缩略图内存 LRU（建议 512 张、单张 ≤ 160 px），与主纹理缓存预算分离。
- [x] 缩略图预加载：预取当前项前后各 3 张，命中缓存时不重新解码。
- [x] Mipmap / 过滤模式随缩放自适应：缩小用 mipmap + LINEAR，100% 附近保持清晰。
- [x] 缩略图条内方向键/滚轮导航，与主画布同步。

**验收**

- 1000 张目录滚动缩略图流畅（60 fps）；冷启动首屏缩略图 1s 内出现。
- 点击相邻图片 < 100 ms 显示（命中纹理或缩略图占位）。
- 3000 张目录内存增长受控（缩略图缓存有明确上限）。

**风险**：显存/内存增长 → 缩略图独立预算 + 磁盘缓存；大目录扫描慢 → 后台增量索引（与 M6 共用）。

### 3.5 P1-M5 发布工程（2~3 d）

> 安装器与文件关联是 v1.0 的硬性门槛；自动更新与多语言可后续补。

- [x] 安装器（Inno Setup 6）：安装、卸载、开始菜单、按用户安装、可选“设为默认看图程序”。
- [x] 文件关联：注册支持的扩展名、ProgID、`OpenWithProgids`、`Applications` 与“默认应用”能力项，资源管理器“打开方式”可用。
- [ ] 自动更新：检查发布清单、下载、哈希/签名校验、失败回滚；默认手动确认（推迟到 v1.1）。
- [ ] 多语言：中/英 UI 资源表（轻量方案优先，如自写 `fl!` 宏或 `rust-i18n`；推迟到 v1.2）。
- [x] 崩溃日志落盘；匿名统计默认关闭且需明示。
- [x] 版本号与 CHANGELOG 规范：`1.0.0` 起语义化版本。

> M5 完成情况：安装器、文件关联、崩溃日志与版本规范已落地；自动更新与中/英 UI 按发布门槛说明拆分到 v1.1 / v1.2。安装器向导暂为英文，应用界面仍为中文。Windows 10/11 不允许安装器静默改写默认看图程序，安装器注册“打开方式”和“默认应用”能力项，并提供系统设置入口。

**验收**

- 干净 Win10/11 虚拟机：安装 → 双击图片 → 关联生效 → 升级 → 卸载无残留。
- 升级失败时保留旧版本可回退；无网络时手动检查更新有明确提示。

**风险**：代码签名证书成本 → 首版可无签名并说明；企业分发/策略管理放 P3。

### 3.6 P1-M6 图库、搜索与对比（3~4 d）

> 只读能力，不修改任何文件；编辑能力不在 P1（见 [PLAN-P2.md](PLAN-P2.md) P2-M8）。排在发布工程之后，但仍属于 v1.0。

- [ ] 图库模式：后台目录索引、网格视图、按名称/日期/尺寸/格式筛选。
- [ ] 搜索：文件名子串、标签；标签写入 sidecar JSON，不污染原文件。
- [ ] 对比模式：2~4 张并排或叠加，同步缩放与平移。
- [ ] 从图库/对比返回查看器时保持缩放、位置与当前图片。
- [ ] 索引失败或目录不可读时降级为普通列表，不阻塞浏览。

**验收**

- 1 万张目录进入图库 < 2s（索引可后台继续），滚动不卡死。
- 对比模式下两张不同尺寸图片可同步缩放、平移。
- 浏览、筛选、对比全程不写入原图或 sidecar 之外的任何文件。

**风险**：范围膨胀 → M6 严格限定为只读浏览/筛选/对比；任何“改文件”的需求一律进入 P2-M8。

### 3.7 P1-M7 GIF / WebP 动画（1~1.5 d，最后做）

> 排在发布工程之后，作为 v1.1 候选；不阻塞 v1.0 发布。

- [ ] GIF / WebP 动画逐帧播放，加载完成前先显示首帧。
- [ ] 播放 / 暂停控制；循环次数遵循文件元数据，异常值兜底。
- [ ] 帧间隔按 GIF delay / WebP duration 计时，异常 delay（0 或超大）钳制到 50~1000 ms。
- [ ] 逐帧上传纹理、及时释放旧帧，复用现有 `Caps` 限制总像素。
- [ ] 与幻灯片共存规则：`Space` 优先控制当前动画；无动画时才控制幻灯片，并在状态栏注明当前绑定。
- [ ] 切换图片、暂停后重置播放状态；动画不参与 LRU 主缓存。

**验收**

- 10 MB GIF 播放不卡顿、不持续增长内存；暂停后 CPU 接近 0%。
- 切换图片立即停止动画；再切回时从首帧重新播放。

**风险**：`image` crate 的动画 API 能力有限 → 先做 1 天 PoC；必要时引入 `gif` / `webp` 专用 crate。

## 4. 依赖与风险

| 依赖 | 说明 |
| --- | --- |
| P0 的 `loader` / `Caps` / `ImageCache` | M7 动画帧、M4 缩略图复用限流与缓存思路 |
| P1-M1 设置持久化 | 为 M5 多语言/更新配置、P2 设置面板提供基础 |
| P1-M2 ICC | 为 P2 的 RAW 与 HDR 色彩链路提供基础 |
| 解码器生态 | M3/M7 受 `image` 与专用 crate 能力、许可、打包体积影响 |

**整体风险**：v1.0 范围较大 → 按里程碑独立验收；查看相关里程碑（M1~M4）优先，发布工程（M5）随后，图库（M6）收尾 v1.0，动画（M7）最后且可延后；编辑已移出 P1，不占用 v1.0 工期。任何新增原生依赖都要先评估安装包体积。

## 5. 验收与发布

- 每个里程碑合并前通过 [ROADMAP.md](ROADMAP.md) 的质量门禁。
- 顺序约束：先完成 M1~M4 的查看能力，再做 M5 发布工程，随后完成 M6 图库（仍属 v1.0）；M7 动画不阻塞 v1.0，编辑不属于 P1。
- 发布前回归 [PLAN-P0.md](PLAN-P0.md) §9.2 手工矩阵中受影响的类别；新增格式/解码器补夹具。
- 性能预算沿用 [PLAN-P0.md](PLAN-P0.md) §10；超出时在验收记录中说明原因。
- 完成后在 `ROADMAP.md` 更新状态、完成日期与“最近完成”。

## 6. 验收记录

### P1-M1 桌面集成与持久化（2026-11-10）

**实现**

- 新增 `settings.rs`：schema 版本（当前 1）、上次目录/文件、排序方式，随 eframe `app.ron` 保存（Windows：`%APPDATA%\image-view\data\app.ron`）。
- 窗口尺寸/位置/最大化由 eframe `persist_window` 持久化；命令行参数优先于上次会话。
- 新增 `fileops.rs`：回收站删除与文件管理器定位；入口为 `Delete` 键、工具栏“文件”菜单、画布右键菜单；`Ctrl+Shift+C` 复制路径。
- 排序支持“名称（自然序）”与“修改时间（新→旧）”，切换后保持当前图片选中。
- 设置记录缺失、损坏或版本不兼容时回退默认值并写日志，不阻塞启动。

**自动化验证**

- `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`（47 个）、`cargo build --release` 通过。
- 单测覆盖：设置往返、缺少字段、损坏内容、版本不兼容回退；会话恢复（文件缺失时退回第一张）；排序切换保持当前项；删除当前项后前进/清空；缓存按路径移除；保存时会话路径转为绝对路径。

**手工验证**

- 无参数启动恢复上次目录与图片：首次浏览 `b.png` 退出后，再次启动日志直接加载 `b.png`；`app.ron` 同时写入窗口几何与 `image_view.settings`。
- `Ctrl+Shift+C` 复制路径：实测剪贴板得到绝对路径。
- `Delete` 删除：文件进入回收站，视图自动前进到下一张（命中预加载缓存）。
- 损坏设置记录由单测覆盖；只读/无权限时 eframe 存储只记录警告，不阻塞启动。
- release exe 约 17.8 MB（较 P0 基线的 16.8 MB 增加约 1 MB，主要来自 `trash` 与 eframe persistence/ron），在可接受范围内。

**未覆盖**

- “在资源管理器中显示”未实际点击（会弹出 Explorer 窗口）；命令构造按平台分支静态检查。
- macOS/Linux 文件管理器定位分支未在实机验证。
- 多显示器拔插后的窗口位置钳制依赖 eframe/winit 行为，未单独测试。

### P1-M2 ICC 色彩管理与 16-bit（2026-11-10）

**实现**

- 新增 `color.rs`：解码器消费前读取内嵌 ICC profile，经 `lcms2` 以相对比色意图转换到 sRGB；JPEG/TIFF/PNG/WebP 均走同一路径。
- 无 profile 时直接按 sRGB 生成纹理；profile 解析失败、转换失败或色彩空间与图像通道不匹配（如灰度 profile 套 RGB 像素）时回退原像素，只写 debug 日志，不阻塞显示。
- RGB/RGBA 走 `RGBA_8` / `RGBA_16` 转换并保留 alpha；灰度走 `GRAY_8` / `GRAY_16` → `RGB_8` / `RGB_16`，手动保留 alpha。
- 16-bit 图片在方向校正、降采样、ICC 转换阶段保持 16-bit，最后一次性四舍五入量化到 8-bit 纹理；显示端仍统一为 RGBA8。
- 新增持久化项 `Settings.show_color_status`（默认关闭），可在“文件”菜单勾选“显示色彩转换标记”；开启后状态栏显示 `Display P3 → sRGB` 等蓝色标记，尺寸栏对 16-bit 源图追加 `· 16-bit`。
- 新增 `examples/color_fixtures.rs`，生成 AdobeRGB / Display P3 / sRGB 参考的 8/16-bit PNG、JPEG、WebP、TIFF 与渐变夹具。

**自动化验证**

- `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`（57 个）、`cargo build --release` 通过。
- 单测覆盖：缺失/损坏 profile 回退、RGB 与灰度转换、alpha 保留、16-bit 精度与量化、profile/通道不匹配回退、PNG 内嵌 ICC 读取、16-bit PNG 解码；设置新增字段缺省时回退默认值。
- release exe 18,941,440 字节（约 18.1 MB，较 M1 增加约 1.1 MB，来自静态链接的 `lcms2`）。

**手工验证**

- `cargo run --example color_fixtures -- target/color-fixtures` 生成 21 张夹具；逐张打开 AdobeRGB / Display P3 的 PNG/JPEG/WebP/TIFF（PNG/TIFF 含 8/16-bit），日志均出现“已转换到 sRGB”及正确的 profile 描述。
- 与 Edge 对同一文件的直接渲染截图对比（各取 14 个色块/渐变采样点）：AdobeRGB 最大通道差 6/255、平均 1.57；Display P3 最大通道差 5/255、平均 2.14，肉眼无可辨差异。
- 16-bit Display P3 与 sRGB 渐变图在 100% 下均无可见色带；状态栏显示 `640×360 · 16-bit`。
- 开启“显示色彩转换标记”后，状态栏显示 `Display P3 → sRGB`；截图与状态栏尺寸标记一致。

**未覆盖**

- CMYK ICC profile 不受支持：`image` 在 LCMS 前已将 CMYK JPEG 解码为 RGB，遇到此类 profile 会回退原像素；未引入 CMYK 专用解码链路。
- 纹理仍为 8-bit（符合 M2 范围），10-bit/HDR 输出留给 P2。
- 未与 Windows“照片”等系统查看器逐屏对比，使用 Edge 浏览器作为色彩管理参照。
- macOS/Linux 未实机验证。

### P1-M3 HEIC / AVIF（2026-11-10）

**实现**

- `loader.rs` 按扩展名分派：`.avif` 走 `imgalifier-avif`（`Av1Backend`），`.heic` / `.heif` 走 `imgalifier-heif`（`HevcBackend`）；通用 `finish` 统一执行尺寸上限、预加载阈值、降采样、位深记录与 ICC → sRGB。
- 两个解码器均为纯 Rust，无 C 工具链、无随包 DLL。HEIC 支持 iPhone 网格照片、裁剪、`irot` / `imir`、alpha 与 4:4:4；AVIF 支持 alpha、网格与序列首帧。
- 解码前先读容器 `info()` 尺寸并执行 250 MP 原图 / 30 MP 预加载阈值检查；容器解析同时设置 `max_pixels`、item 数与单 item 字节上限。
- HEIC/AVIF 共用的 `colr` ICC 读取：有 profile 时经 `lcms2` 转换到 sRGB，无 profile 时保留解码器输出。
- 新增 Cargo 特性 `avif`、`heic`（默认开启，可单独关闭）；关闭对应特性时返回 `MissingDecoder`，错误横幅明确提示重新构建。
- 目录扫描与文件对话框加入 `avif` / `heic` / `heif` 扩展名。

**自动化验证**

- `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`（61 个）、`cargo build --release` 通过；`cargo test --no-default-features` 同样通过。
- 单测覆盖：扩展名分派（大小写）、损坏 AVIF/HEIC 的错误类型、缺失解码器文案、无特性构建的 `MissingDecoder` 路径、目录扩展名识别。
- release exe 21,053,440 字节（约 20.1 MiB），较 M2 的 18,941,440 字节增加 2,112,000 字节（约 2.0 MiB）；`--no-default-features` 为 18,946,048 字节。
- 许可：`imgalifier-heif` MIT；`imgalifier-avif` MIT AND BSD-2-Clause（内含 vendored `rav1d`），无 AGPL 依赖。

**手工验证**

- iPhone 网格 HEIC（`IMG_4862.HEIC`，4284×5712，内嵌 Display P3 ICC）在 12% 适配显示；按 `R` 顺时针旋转后状态栏变为 17%，内容方向正确。
- 同目录 `01-iphone.HEIC` → `02-avif.avif`（800×533）→ `03-libheif.heic`（1280×854）依次翻页，状态栏 `1/3` → `2/3` → `3/3`，按 `←` 返回 `2/3`；三张均正常显示。
- 额外样例解码通过：灰度 `grey.heic`、alpha `alpha.heic`、4:4:4 `yuv444.heic` / `rgb444-lossless.heic`、`transforms-odd.heic`、`heif-bitcoin-color.heif`（2880×1800）、`heif-lines.heif`（1920×1920）。
- iPhone 样例的 ICC 转换日志为 `Display P3 → sRGB`；状态栏色彩标记开启后可见。
- `--no-default-features --release` 构建分别打开 `.avif` 与 `.heic`：横幅显示“AVIF/HEIC 解码器未包含在此构建中；请使用启用该特性的版本打开 .avif/.heic 文件”，窗口保持可用，无崩溃。

**未覆盖**

- 未与系统查看器逐像素比较 HEIC/AVIF 色彩，只验证了 ICC 识别、转换日志与目视结果；AVIF 样例未内嵌 ICC。
- 仅带 nclx 原色（如无 ICC 的 Display P3）的 HEIC/AVIF 暂不做色域转换；10/12-bit 与 HDR（PQ/HLG）源图由解码器内部转为 8-bit，未做 HDR 输出。
- `.heif` 扩展名统一按 HEIC 处理；若 AVIF 使用 `.heif` 扩展名会解码失败并显示错误，可用 `.avif` 扩展名打开。
- HEVC（H.265）专利许可未评估，商业分发前需自行确认。
- macOS/Linux 未实机验证。

### P1-M4 缩略图条与渲染质量（2026-11-10）

**实现**

- 新增 `thumbs.rs`：底部 96 px 缩略图条，横向 `ScrollArea` + 手算可见范围虚拟滚动，只创建视口内卡片；点击选中、悬停显示文件名，方向键翻页或点击后自动把当前项滚动到中央。
- 独立缩略图解码线程，2 张并行；请求按“当前项 → 前后各 3 张 → 视口其余”排序。新的视口请求会替换尚未开始的旧队列，快速滚动时不会积压旧缩略图。
- 缩略图解码复用 `loader::decode_thumbnail`，沿用主图的格式分派、ICC → sRGB、尺寸/像素上限与降采样逻辑；目标最长边 128 px（满足 ≤160 px 建议并支持 2× DPI）。
- 缩略图内存 LRU 上限 512 张，与主图 `ImageCache` 预算分离；主图与缩略图纹理均使用 mipmap + LINEAR，缩小更平滑，100% 附近仍保持清晰。
- 磁盘缓存位于 `%LOCALAPPDATA%\image-view\thumbs`：QOI 文件，键 = 绝对路径 + 修改时间 + 文件长度 + 目标尺寸（FNV-1a 64）；临时文件 + rename 原子提交，启动时清理到 256 MB；读取时校验 QOI 头与尺寸，损坏项自动删除。
- eframe 渲染后端从默认 wgpu 切换为 glow（egui-wgpu 不支持 `mipmap_mode`）；release 体积由 M3 的 21.05 MB 降至 15.16 MB。
- 删除当前图片时同步移除其缩略图内存项；切换目录时递增 generation 并取消旧请求，旧结果不会写入新目录。

**自动化验证**

- `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`（70 个）、`cargo test --no-default-features`（70 个）、`cargo build --release` 通过。
- 单测覆盖：缩略图 LRU 淘汰与命中刷新、可见范围虚拟化、磁盘缓存往返与损坏清理、缓存键包含路径/尺寸、预算清理、mipmap 采样选项、缩略图最长边限制、缩略图选中、Focus 请求替换/去重/忽略旧 generation。
- release exe 15,161,856 字节（约 14.5 MiB），较 M3 的 21,053,440 字节减少 5,891,584 字节，主要来自移除 wgpu 后端。

**手工验证**

- 1000 张 1600×1200 JPEG：清空独立 `%LOCALAPPDATA%` 后冷启动，首批 14 张缩略图（覆盖首屏 12 张）在解码线程启动后 845 ms 内完成；当前项排在最前，选中缩略图更早出现。二次启动命中磁盘缓存，首屏无等待感。
- 以 16 ms 间隔连续发送 500 次 `→`（≈60 fps 输入）时，临时帧统计平均 16.6~16.8 ms、最大约 27~35 ms；缩略图条跟随当前项滚动，停止后 2~3 s 内可见缩略图全部补齐，无占位符残留。
- 点击当前项右侧相邻缩略图，窗口标题与画面切换实测 4.7~52.9 ms（命中预加载时约 5 ms），低于 100 ms。
- 3000 张 640×480 JPEG：清缓存首屏进程工作集约 92 MB；快速浏览全部 3000 张过程中工作集高水位约 430 MB，继续浏览不再增长。纹理管理器分配数在 500 张后稳定为 516（512 张缩略图 + 主图等），浏览到 1000 / 1500 / … / 3000 张期间保持不变，说明 LRU 上限生效；工作集数值受 Windows 堆与显卡驱动高水位影响。
- 1000 张首屏实测生成 15 个 QOI 缓存文件，共 40,842 字节；二次启动直接读取，未重新解码。

**未覆盖**

- 自动化脚本注入的系统滚轮事件未送达 egui，缩略图条滚轮横向滚动仅按 `ScrollArea` 行为实现，未做自动验收；方向键跟随与点击已验证。
- 60 fps 帧间隔使用临时 debug 统计，验收后已移除，未在 release 构建下重复采样。
- 缩略图/主图纹理的 mipmap 会增加约 33% 显存；当前只统计纹理分配数量与进程工作集，未逐张计量 GPU 显存。
- 128 px 缩略图在极低 DPI 或非整数缩放显示器上可能略显模糊，尚未做多显示器 DPI 矩阵回归。
- macOS/Linux 未实机验证。

### P1-M5 发布工程（2026-11-10）

**实现**

- 新增 `logging.rs`：常规日志写入 `%LOCALAPPDATA%\image-view\logs\image-view.log`，同时输出 stderr；单文件超过 4 MiB 轮转为 `.1`~`.3`。启动时记录版本、系统与架构；可用 `IMAGE_VIEW_LOG_DIR` 覆盖目录。
- panic hook 在常规日志之外写独立 `crash-*.log`，包含 UTC 时间、panic 位置、消息与 `Backtrace::force_capture()`；最多保留 10 份。release 的 `strip` 从 `symbols` 调整为 `debuginfo`，保留函数名供 backtrace 使用。
- “文件”菜单新增“打开日志文件夹”，直接调用系统文件管理器打开日志目录。
- 新增 Inno Setup 6 脚本 `installer/image-view.iss`：按用户安装到 `%LOCALAPPDATA%\Programs\image-view`（无需管理员），x64 / Windows 10+，开始菜单快捷方式、可选桌面图标、可选“设为默认看图程序”。
- 文件关联：为 18 个扩展名注册 ProgID `ImageView.Image`、`OpenWithProgids`、`Applications\image-view.exe\SupportedTypes`，以及 `RegisteredApplications` + `Capabilities\FileAssociations` 默认应用能力项；卸载清理全部新增键值。
- 默认查看器任务在安装前通过 Inno Pascal 备份 `HKCU\Software\Classes\.<ext>` 的原默认值，卸载时仅在当前值仍为 `ImageView.Image` 时恢复，避免覆盖用户安装后手动选择的查看器；同时提供安装完成后打开 Windows“默认应用”设置的入口。
- 新增 `scripts/build-release.ps1`：从 `Cargo.toml` 读取版本，执行 `cargo build --release --locked`，调用 ISCC 生成安装包，并输出 `dist\SHA256SUMS.txt`。
- 生成 16/24/32/48/64/128/256 多尺寸 `assets/icon.ico`；`build.rs` 在 Windows MSVC 下把 `assets/app.res`（仅图标资源）链接进 exe，使 exe、快捷方式与安装包使用同一图标。
- 版本统一为 `1.0.0`（发布前所有开发工作都归入首个正式版）；新增 `CHANGELOG.md`，记录 Keep a Changelog 结构、语义化版本与安装包版本映射规则。
- 卸载同时删除 `%APPDATA%\image-view`（窗口/会话设置）与 `%LOCALAPPDATA%\image-view`（日志、缩略图缓存），避免用户数据残留。
- 不引入遥测或联网上报；匿名统计默认关闭（本版本没有统计代码），日志仅保存在本机。

**自动化验证**

- `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`（75 个）、`cargo test --no-default-features`（75 个）、`cargo build --release` 通过。
- 新增单测：UTC 时间换算、日志轮转与档位后移、小日志不轮转、崩溃转储按名称保留最新 10 份、日志目录创建与追加写入。
- release exe 15,262,720 字节（约 14.56 MiB），较 M4 的 15,161,856 字节增加 100,864 字节。
- 安装包 `image-view-1.0.0-setup.exe` 6,513,568 字节，SHA256 `2019938233dc1a84b9771a3fd9e874adfc2f24fdceed33f8330f55960d0bbd9a`。
- 版本统一为 `1.0.0` 后重建并复验：静默安装退出码 0，卸载项 `DisplayVersion` 为 `1.0.0`；静默卸载退出码 0，安装目录与卸载项均移除。
- 临时 panic 自检（验收后已移除）：`IMAGE_VIEW_TEST_PANIC=1` 启动后退出码 101，生成 3,849 字节 `crash-*.log`，包含版本、UTC 时间、`src\main.rs:27:9`、消息与 backtrace。

**手工验证**

- 静默安装 `/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /TASKS="setdefault"`：退出码 0，程序、README、CHANGELOG、卸载器写入 `%LOCALAPPDATA%\Programs\image-view`。
- 注册表检查：`HKCU\Software\Classes\ImageView.Image`、`Applications\image-view.exe`、`Software\image-view\Capabilities` 与 `RegisteredApplications\image-view` 均存在；合并视图 `HKCR\ImageView.Image\shell\open\command` 指向安装后的 exe 与 `%1`。
- `.png` / `.qoi` / `.avif` 的 `OpenWithProgids` 均含 `ImageView.Image`；选择默认查看器任务后三者默认值变为 `ImageView.Image`。
- 用 ShellExecute 打开 `target\assoc-test\test.qoi`（无 `UserChoice`，故走默认 ProgID）：实际启动 `%LOCALAPPDATA%\Programs\image-view\image-view.exe`，说明关联命令生效。
- 连续两次同版本覆盖安装（模拟升级）后默认值仍为 `ImageView.Image`；静默卸载退出码 0，安装目录、ProgID、Applications、Capabilities、`Software\image-view`、`RegisteredApplications` 值、`%APPDATA%\image-view`、`%LOCALAPPDATA%\image-view` 全部消失。
- 卸载后 `.png` / `.qoi` / `.avif` 的默认值恢复为安装前的空值，`OpenWithProgids` 中的 `ImageView.Image` 值被移除。
- 通过 `ExtractAssociatedIcon` 提取 release exe 图标，确认是 image-view 的多尺寸图标而非 Rust 默认图标。

**未覆盖**

- 没有干净的 Win10/11 虚拟机，未完成“全新系统安装 → 双击 → 升级 → 卸载”的完整 VM 验收；本机已完成等价的静默安装/覆盖安装/卸载与注册表清理验证。
- 安装包未签名，SmartScreen“未知发布者”提示与下载信誉未验证。
- Windows 10/11 通过 `UserChoice` 锁定常见扩展名（本机 `.png` 仍为“照片”），安装器的默认查看器任务只写 HKCU 默认值并提供设置入口，无法静默夺取默认；最终仍由用户在“默认应用”中确认。
- 安装器向导暂为英文；中/英 UI 资源表与自动更新按计划推迟到 v1.2 / v1.1。
- 仅验证覆盖安装（同版本重装），未验证跨版本升级中的设置/缓存兼容；仅构建 x64，未验证 ARM64/x86。
- 默认应用能力项在 Windows“设置 → 默认应用”中的显示与排序未逐项截图确认。
- Inno Setup 6.7 编译器在本地构建时标记 `Non-commercial use only`；若用于商业分发，需先确认 Inno Setup 授权或改用 WiX。

## 7. 变更记录

| 日期 | 变更 |
| --- | --- |
| 2026-11-10 | 从 `PLAN-P0.md` 的远期章节拆出，独立为 P1 计划；定义 M1~M7 |
| 2026-11-10 | P0 阶段未覆盖的窗口持久化、Mipmap、回收站/复制路径、文件夹位置并入 M1 / M4 |
| 2026-11-10 | 调整优先级：编辑从 M6 拆出为 M8 并移到最后；M6 收敛为只读的图库、搜索与对比 |
| 2026-11-10 | 编辑移出 P1：P1 收敛为 M1~M7（查看 + 发布），编辑归入 [PLAN-P2.md](PLAN-P2.md) P2-M8 |
| 2026-11-10 | 动画从 M2 移到 M7（排在发布工程之后），原 M3~M7 顺延为 M2~M6 |
| 2026-11-10 | 发布工程与图库互换：发布工程提前为 M5，图库、搜索与对比后移为 M6 |
| 2026-11-10 | 明确图库（M6）仍属 v1.0，仅执行顺序后移；v1.0 门槛为 M1~M6 全部完成 |
| 2026-11-10 | 完成 P1-M1：窗口/会话/排序持久化、回收站删除、复制路径、资源管理器定位 |
| 2026-11-10 | 完成 P1-M2：ICC → sRGB 转换、16-bit 保持精度、色彩转换状态标记与验收夹具 |
| 2026-11-10 | 完成 P1-M3：HEIC/AVIF 纯 Rust 解码、缺失解码器提示、特性开关与验收记录 |
| 2026-11-10 | 完成 P1-M4：缩略图条/虚拟滚动、磁盘缓存与内存 LRU、预加载、mipmap + glow 渲染、验收记录 |
| 2026-11-10 | 完成 P1-M5：Inno Setup 安装器、文件关联/默认应用、崩溃日志、图标与版本/CHANGELOG；自动更新与多语言推迟到 v1.1/v1.2 |
| 2026-11-10 | 版本口径统一：发布前所有工作归入 `1.0.0`，原 `2.0.0-alpha.1` / `v2.0` / `v2.1` / `v2.2` 分别改为 `1.0.0` / `v1.0` / `v1.1` / `v1.2` |
