# image-view · UI 重设计（redesign 分支）独立 QA 回归与验收报告

> 执行人：严过关（Yan）／QA 工程师
> 工作树：`C:/Users/99160/WorkBuddy/Worktrees/image-view/redesign`（分支 `redesign`，HEAD `06bb075`）
> 工具链：cargo 1.99.1 / rustc 1.99.1；`CARGO_TARGET_DIR=E:/private/Rust/image-view/target`、`CARGO_INCREMENTAL=0`
> 依据文档：`docs/UI-REDESIGN-PRD.md`（§5 规格 / §6 不变更清单 / §7 验收标准）、`docs/UI-REDESIGN-DESIGN.md`（§8 跨文件约定 / §9 风险 / §10 可执行验收清单）
> 本报告全部结论为独立复核所得；未采信工程师自检结论作为结论，仅作为对照。

---

## 0. 结论速览

> **Round 2 回归（修复后验证，2026-11-13）**：工程师已按 QA-REDESIGN-001（方向 a：双 Area 拆分）修复 `overlay.rs` 的 `draw_all`；QA 完成独立回归，路由判定 **`NoOne`（成功）**。本轮新增 2 项穿透测试，门禁 5/5 全绿，原 P0 缺陷复现测试转绿，总计 71 项测试全绿。

| 项 | 结论 |
| --- | --- |
| **智能路由判定** | **`NoOne`（修复后回归全绿，判定成功）** |
| 核心缺陷（已修复） | `overlay.rs:80` 旧 `Area::interactable(false)` 使 `overlay.rs:294/313` 关闭／重试按钮永远收不到点击；工程师已改为双 Area：绘制型浮层 `interactable(false)`（toast / 文件名胶囊）+ 交互型错误卡片 `interactable(true)` 且仅当 `error.is_some() && alpha>0` 时以卡片包围盒创建（overlay.rs:160-176），违反 PRD B7 的根因已消除 |
| 实证测试（修复后） | `error_card_close_button_is_reachable_by_a_real_click` 已由 `None` 转绿为 `Dismiss`；另新增 `canvas_click_passes_through_when_no_error` 与 `canvas_click_passes_through_when_error_alpha_is_zero` 证明无错误 / alpha≤0 时画布点击穿透、错误 Area 不创建 |
| 5 项门禁（补测前基线） | 5/5 全绿，退出码均为 0 |
| 5 项门禁（Round 2 修复后回归） | **5/5 全绿，退出码均 0**：fmt / clippy(-D warnings) / check 全绿；`test -p image-view` 71/71、`test --workspace` 全绿（含 doc-tests）、`test --workspace --no-default-features` 退出码 0（仅 1 条既有 dead_code 温和 warning，非 error） |
| 新增自动化测试 | 原 35 项现 35/35 通过 + **Round 2 新增 2 项穿透测试**，合计 **37 项新增全绿**；总计 71 项（原有 34 + 新增 37）**71/71 通过** |
| 零回退核对 | **通过**（含本轮再确认）：改动集合恰 8 改 + 2 新增源码 + 2 新增 docs；本轮仅 `overlay.rs` 的 `draw_all` 交互逻辑变动，未波及其余 9 个 UI 文件、禁改文件与依赖树 |
| 验收清单 | 逐条核对 39 项（PRD §7 共 37 条 + DESIGN §10 附加 C10/D9）；**B7 已转为通过**，合计 23 通过 / 5 部分 / 11 需 GUI / 0 不通过 |
| 人工 GUI 复核 | 本环境无法运行 GUI，**C1~C7 的 16 组合矩阵等共 20 类项目仍全部列为需人工截图**，未伪装通过 |
| 测试轮次 | Round 1（测试代码自修 → 探针实证定位 P0）＋ Round 2（修复后回归验证）＝完整闭环；按 SOP 退出，遗留项见 §8.2 |

> **重要说明**：QA **未修改任何产品逻辑**。所有补充测试以 `#[cfg(test)] mod qa_redesign_tests` 追加在 6 个已改动/新增的源码文件内（未新增测试文件）；`git status` 的 UI 实现改动集合在测试追加前后保持 12 项不变。本报告 `docs/UI-REDESIGN-QA.md` 是任务书明确允许的 QA 阶段产物，落盘后工作树共 13 项（12 项 UI 改动 + 1 份 QA 报告），**不计入 §2.1 的零回退集合**。工作树中 `cargo test --workspace` 变红是**新增失败回归测试故意暴露源码缺陷**所致，工程师修复后即自动转绿，请勿删除或 `#[ignore]` 该测试。

---

## 1. A. 构建与静态门禁

### 1.1 补测前基线（复核工程师自检，全部独立重跑）

| # | 命令 | 退出码 | 关键输出 |
| --- | --- | --- | --- |
| 1 | `cargo fmt --all -- --check` | **0** | 无 diff |
| 2 | `cargo test --workspace` | **0** | `image-view` 34/34；各 crate 7/7、7/7、1/1、2/2、25/25、10/10、8/8、1/1 全绿；13 个 crate doc-tests 全绿 |
| 3 | `cargo test --workspace --no-default-features` | **0** | 全绿（17 个 decode 用例因特性关闭而减少）；1 条 `dead_code` warning，根因见 §1.3 |
| 4 | `cargo clippy --workspace --all-targets -- -D warnings` | **0** | 0 error / 0 warning |
| 5 | `cargo check -p image-view` | **0** | `Finished dev profile ... in 2.89s` |

### 1.2 QA 补测后最终状态（35 项新增测试入库后）

| # | 命令 | 退出码 | 关键输出 |
| --- | --- | --- | --- |
| 1 | `cargo fmt --all -- --check` | **0** | 无 diff（QA 测试代码已由 `cargo fmt --all` 格式化） |
| 2 | `cargo test --workspace` | **101** | `running 69 tests` → `68 passed; 1 failed`；唯一失败：`error_card_close_button_is_reachable_by_a_real_click` |
| 3 | `cargo test --workspace --no-default-features` | **101** | 同上 68/1；另有 1 条既有 `dead_code` warning |
| 4 | `cargo clippy --workspace --all-targets -- -D warnings` | **0** | 0 error / 0 warning（含新增测试代码） |
| 5 | `cargo check -p image-view` | **0** | 0 error / 0 warning |
| 附 | `cargo test -p image-view` | **101** | 68/69；`left: None / right: Dismiss` |

> 结论：静态门禁（fmt / clippy / check）**全部通过**；测试门禁的失败**唯一来源**为本次 QA 新增的缺陷复现测试，与原有 34 个测试无关，也非零回退回退。

> **Round 2 更新**：该 `101` 已在工程师修复 `overlay.rs` 后消除（详见 §1.5）。修复后 `cargo test -p image-view` 为 71/71、`cargo test --workspace` 全绿、`cargo clippy --workspace --all-targets -- -D warnings` 全绿，交付前门禁恢复 5/5 全绿。

### 1.3 Warning 根因查证（不未查证即标注「与本次改动无关」）

`cargo test --workspace --no-default-features` 输出：

```
warning: variants `LoadAvif`, `LoadHeic`, `LoadSvg`, and `SvgzUnsupported` are never constructed
   --> apps\eframe-app\src\i18n.rs:131:5
```

查证过程与结论：
- `apps/eframe-app/src/errors.rs`（**零 diff**）中 `LoadError::Avif/Heic/Svg/SvgzUnsupported` 分支分别由 `#[cfg(feature = "avif" / "heic" / "svg")]` 条件编译（errors.rs:19-29）；关闭默认特性后这些分支不存在，对应 `Key` 变体失去构造点。
- `i18n.rs` 的 `Key` 枚举与 `zh()`/`en()` 两张资源表**逐字零 diff**，属于既有结构。
- 因此这是**既有条件编译现象**，与 UI 重设计无关；且为 warning 而非 error，门禁退出码 0（补测前）。`-D warnings` 仅作用于 clippy 命令（默认特性），不受影响。

### 1.4 工程师自检结论复核

| 自检项 | QA 复核 |
| --- | --- |
| fmt / check / clippy 通过 | ✅ 独立重跑确认 |
| `cargo test -p image-view` 34/34 | ✅ 确认（补测前 34/34） |
| `cargo test --workspace` 全绿含 doc-tests | ✅ 确认（补测前） |
| 共享 target 的 Windows 增量锁 `os error 5` 噪声，`CARGO_INCREMENTAL=0` 后消失 | ✅ 本次全程使用 `CARGO_INCREMENTAL=0`，未复现 |
| 首次前台超时被 SIGTERM，后台重跑通过 | ✅ 本次全程后台执行，无超时 |
| 全局一致性审查 IS_PASS: YES | ⚠️ **不予采信**：QA 独立发现 1 个 P0 级交互缺陷（§8），工程师的全局审查未覆盖 `Area::interactable(false)` 与子 `ui.interact` 的语义冲突 |

---

## 1.5 Round 2 修复后回归（2026-11-13）

工程师按 QA-REDESIGN-001（方向 a：双 Area 拆分）修复 `apps/eframe-app/src/overlay.rs` 的 `draw_all`：绘制型浮层 `canvas_overlays` 保持 `interactable(false)`（仅承载 toast / 全屏文件名胶囊）；交互型错误卡片 `error_card_overlay` 改为 `interactable(true)`，且仅当 `state.error.is_some() && error_alpha(elapsed) > 0.0` 时以 `fixed_pos` + `default_size` 将 Area 严格收缩到卡片包围盒创建（overlay.rs:160-176）。QA 独立完成以下回归，路由判定见 §8.1。

### 1.5.1 5 项门禁回归（退出码全 0）

| # | 命令 | 退出码 | 关键输出 |
| --- | --- | --- | --- |
| 1 | `cargo fmt --all -- --check` | **0** | 无 diff |
| 2 | `cargo test -p image-view` | **0** | `running 71 tests` → `71 passed; 0 failed`（原复现测试转绿 + Round 2 新增 2 项穿透测试） |
| 3 | `cargo test --workspace` | **0** | 各 crate 与 doc-tests 全绿（含 image-view 71/71） |
| 4 | `cargo test --workspace --no-default-features` | **0** | 退出码 0；仅 1 条既有 `dead_code` 温和 warning（`errors.rs` 条件编译现象，非 error，见 §1.3） |
| 5 | `cargo clippy --workspace --all-targets -- -D warnings` | **0** | 0 error / 0 warning（含全部新增测试） |
| 附 | `cargo check -p image-view` | **0** | 0 error / 0 warning |

> 结论：**5/5 门禁全绿**，原 P0 缺陷复现测试（`error_card_close_button_is_reachable_by_a_real_click` 期望 `Dismiss`、实际 `None`）已转绿，K1 / K2 遗留项关闭。

### 1.5.2 零回退再确认（本轮仅 overlay.rs 变动）

- `git status --porcelain` 恰好 13 项（8 改 + 2 新增源码 + 2 新增 docs + 1 QA 报告），无新增 / 越界文件；
- 禁改文件 `i18n.rs` / `errors.rs` / `document.rs` / `cache.rs` / `update.rs` / 两个 `Cargo.toml` / `Cargo.lock` / `crates/*` / `design/*` / `README.md` / `rust-toolchain.toml` 相对 HEAD **全部零 diff**；
- `Cargo.lock` 零 diff，未新增任何依赖；
- 文件 mtime 证据：仅 `overlay.rs`（16:11）为本轮改动，其余 9 个 UI 源码 mtime 均早于本轮（13:42–15:41），佐证修复仅落在 `overlay.rs` 的 `draw_all`。

### 1.5.3 Round 2 新增测试（QA-REDESIGN-002 补充断言）

在 `overlay.rs::qa_redesign_tests` 新增 2 项穿透测试（不删除任何既有测试）：

| 测试 | 覆盖 | 结果 |
| --- | --- | --- |
| `canvas_click_passes_through_when_no_error` | error=None 时 `error_card_overlay` 不创建，画布层控件收到点击（B7 互补约束：无错误时不拦截画布） | ✅ 通过 |
| `canvas_click_passes_through_when_error_alpha_is_zero` | `since` 置于未来 → `elapsed` 饱和到 0 → `error_alpha(0)=0` → alpha≤0 时不创建 error Area，画布点击穿透 | ✅ 通过 |

---

## 2. B. 零回退独立核对

### 2.1 改动集合（基线陷阱说明）

`git diff main --name-status` 会额外出现 `.gitignore`，易被误判为越界改动。查证结果：

```
main   = 172f2f7 "chore: 更新 gitignore 文件"
HEAD   = 06bb075 "build: 让 cargo-deny 检查通过"   （redesign 分支基点）
merge-base(main, HEAD) = 06bb075
```

即 **main 分支领先 redesign 一个 commit**；`.gitignore` 差异来自基线分叉（`target/ → /target` 等写法变化），并非本次工作树改动。正确基线应为 `HEAD`（`git diff HEAD` 对比工作树）。

`git diff HEAD --name-status` + `git ls-files --others --exclude-standard` 得到**真实改动集合恰好 12 项**：

| 类别 | 文件 |
| --- | --- |
| 修改（8） | `app.rs`、`icons.rs`、`main.rs`、`theme.rs`、`thumbs.rs`、`titlebar.rs`、`ui.rs`、`viewer.rs` |
| 新增源码（2） | `overlay.rs`、`visuals.rs` |
| 新增文档（2） | `docs/UI-REDESIGN-DESIGN.md`、`docs/UI-REDESIGN-PRD.md` |

禁止出现的文件全部**零 diff**（`git diff HEAD -- ...` 为空）：`i18n.rs`、`errors.rs`、`document.rs`、`cache.rs`、`update.rs`、`crates/*`、两个 `Cargo.toml`、`Cargo.lock`、`design/redesign/*`、`README.md`、`rust-toolchain.toml`。`main.rs` 仅 +2 行（`mod overlay;` / `mod visuals;`），符合 DESIGN F8。

### 2.2 零回退逐项核对表

| # | 核对项 | 方法 | 结果 |
| --- | --- | --- | --- |
| 1 | `i18n.rs` Key 枚举 + 中英资源表逐字未改 | `git diff HEAD -- i18n.rs` | ✅ 空 diff |
| 2 | `errors.rs` 零改动 | `git diff HEAD` | ✅ 空 diff |
| 3 | Settings schema 未新增字段 | `crates/*` 零 diff；`git diff HEAD -U0 -- app.rs \| grep settings\.` 无输出 | ✅ 无变化；`settings::tests` 10 项含损坏/过时/缺字段回退全通过 |
| 4 | Cargo 依赖树零变化 | `Cargo.toml` / `apps/eframe-app/Cargo.toml` / `Cargo.lock` 空 diff；`glow`、`egui=0.37` 保持 | ✅ |
| 5 | 16 条快捷键 `consume_key` 完整 | 基线 HEAD 与工作树 `handle_shortcuts` 函数体 diff：仅 3 行 `fullscreen_title_until → fullscreen_title_since`（三段计时所需内部字段重命名），**所有 `consume_key` 键位/修饰键/动作行零变化** | ✅ 键位逐字未改；R/Shift+R 由 `consume_rotation` 处理，双击/滚轮由 `viewer`/`thumbs` 处理，均未改 |
| 6 | 幻灯片 3s | `app.rs:31 SLIDESHOW_INTERVAL = 3s`，diff 未触及 | ✅ |
| 7 | 短提示 ≈4s | `NOTICE_DURATION = 4s`；`tick_notice` 取 `max(4s, motion.toast_total_ms()=4000ms)` | ✅ 4.0s（Z1 口径） |
| 8 | 全屏文件名 3s | `FULLSCREEN_TITLE_DURATION = 3s`；`motion.title_total_ms()=3000` | ✅ |
| 9 | Fit 5% 留白 | `crates/iv-image-render/src/lib.rs:12 const FIT_PADDING: f32 = 0.9;`（跨 crate 零 diff） | ✅ |
| 10 | 滚轮横向滚动缩略图 | `thumbs.rs:452-457` `always_scroll_the_only_direction` + `ScrollArea::horizontal()`，diff 未触及 | ✅ |
| 11 | MAX_SIDE 128 / 512 LRU / 256MB / ±3 预取 / 3 张主缓存 | `thumbs.rs:17/19/21`、`cache.rs:13/15`（3 张 / 72M 像素）、`app.rs:410-424 warm_thumbnails(1..=3)`，diff 关键行零变化 | ✅ |
| 12 | generation 取消 | `app.rs` `generation` / `thumbnail_generation` 逻辑零 diff；`generation_advances_and_rejects_stale_results` 单测通过 | ✅ |
| 13 | 大图阈值 8192 / 40MP / 250MP / 30MP | `crates/iv-image-decode/src/lib.rs:30-33` 现值逐项一致 | ✅ |
| 14 | mipmap + LINEAR / glow | `thumbs.rs:82`、`document.rs:30`、`apps/eframe-app/Cargo.toml:32 "glow"` | ✅ |
| 15 | `error_banner` 的 `Panel::top` 已删除 | 基线 `ui.rs:696 Panel::top("error_banner")`；当前 grep 仅剩 `app.rs:1145` 注释；`grep -rn "error_banner"` 无实现 | ✅ 已删除 |
| 16 | 无新增占布局 Panel | 基线 Panel：`ui.rs:26 toolbar(top)`、`ui.rs:610 status_bar(bottom)`、`titlebar.rs:58 titlebar(top)`、`thumbs.rs:438 thumbnail_strip(bottom)`；当前仅这 4 个（行号 69/669/60/441），集合完全一致 | ✅ 无新增；浮层全部为 `Area(Order::Middle)` |
| 17 | `design/redesign/*` / `README.md` 未改 | `git diff HEAD` 空 | ✅ |
| 18 | 依赖树 / 渲染后端 / 打包 / 文件关联 | 相关文件零 diff | ✅ |

> **观察项（非 Bug）**：工程师把 `thumbs::tests::visible_range_virtualizes_large_lists` 的硬编码期望 `0..11` 改写为「由 `ITEM_STEP` 派生」的表达式，虽符合 DESIGN §9.1 R-8 的建议，但会让断言退化为与实现同义反复，独立验证力下降。QA 已补 `visible_range_follows_the_92pt_step_with_hardcoded_expectations`（用规格常数 92 硬编码）弥补，建议保留。

---

## 3. C. 补充自动化测试

### 3.1 测试清单（35 项，全部以项目现有 `#[cfg(test)]` 风格追加，未新增文件）

| 模块 | 测试数 | 覆盖内容 | 结果 |
| --- | --- | --- | --- |
| `visuals.rs` | 17 | `snap`/`snap_rect`/`snap_radius` 在 1x/1.25x/1.5x/2x 取整；`snap_line_center` 整数/分数 ppp；`snap_stroke`≥1 物理像素；`resolve_state` 六态互异、浅深两主题下开启态≠hover 底色；`three_phase_alpha` 三段与 alpha≤0/0 时长边界；`error_alpha` 淡入后常驻；`Motion` 4000/3000ms、≤600ms、`reduced()` 降级；`overlay_geometry` 错误卡片与文件名胶囊避让、toast 右下、尺寸 clamp；规格尺寸常量；`stable_id` 确定性；`chrome_palette` @72%；`motion(ctx)` 降级 | 17/17 通过 |
| `theme.rs` | 6 | 浅/深 S0<S1<S2≤S3 明度阶梯；深色 S1−S0≥25（实测 32.9）；Q3 强调色 `#2F6BFF`/`#5B8CFF`；`chrome=strip=status`；mask=184(72%)、四层阴影递增；`Palette::of` | 6/6 通过 |
| `icons.rs` | 2 | 22 个 `Icon` 变体全部 `paint` 不 panic（headless egui）+ 退化尺寸；20/18/1.6 常量 | 2/2 通过 |
| `overlay.rs` | 6 | **egui `Area::interactable(false)` 语义探针**（对照组 true 可点 / false 不可点）；**错误卡片关闭按钮真实点击可达性**（修复后转绿）；Round 2 新增 **2 项画布穿透断言**（无错误 / alpha≤0 时不创建 error Area、画布点击穿透）；自绘复选/单选/菜单项全状态绘制不 panic；S3 菜单尺寸常量 | 6/6 通过 |
| `thumbs.rs` | 3 | `ITEM_STEP=92` 与 `80+12` 派生、`MAX_SIDE/512/256MB/2 线程` 零回退；`visible_range` 用硬编码 92 的独立期望；`item_rect` 92 网格 | 3/3 通过 |
| `ui.rs` | 3 | 顶栏/缩放/节奏尺寸常量；`icon_button` 六态映射（开启态≠hover、焦点环、禁用）；`human_size` | 3/3 通过 |
| **合计** | **37** | | **37 通过 / 0 失败** |

统计：`cargo test -p image-view` 共 71 项（原有 34 + 新增 37），**71 通过 / 0 失败**，通过率 100%；新增测试通过率 100%。（原 35 项新增中 1 项缺陷复现测试在工程师修复 `overlay.rs` 后转绿，本轮再新增 2 项穿透测试。）

### 3.2 测试轮次与路由记录（严格遵守 2 轮上限）

| 轮次 | 动作 | 结果 | 判定 |
| --- | --- | --- | --- |
| Round 1 | 写入 35 项测试并运行 | 63/69；6 项失败全部为 `Dropped TexturesDelta with 1 unapplied deltas` | **QA（测试代码 Bug）**：egui `FullOutput` 被直接 drop；自行修复为 `run_ui()` 辅助函数调用 `drop_without_applying_deltas()` |
| Round 2 | 回归（self-fix 后） | 67/69；2 项失败，且对照组 `interactable(true)` 也不可点 | 判定为**探针基础设施缺陷**：`Area` 首帧 sizing pass 会临时把 layer 标记为不可交互（`area.rs:538`），press/release 需分帧注入。修正帧序后做**一次决定性确认** |
| Round 2 确认 | 探针修正后最终验证 | 68/69：语义探针通过；产品期望测试失败 `None` vs `Dismiss` | **Engineer（源码 Bug 实证）**；按 2 轮硬上限退出，不再进入 Round 3 |
| Round 2 回归 | 工程师修复 `overlay.rs` 双 Area 后 | 71/71 全绿（复现测试转绿 + 新增 2 穿透测试）；5 项门禁 5/5 全绿 | **NoOne（成功）**：仅测试代码补充，源码 Bug 已修复 |

> 全程产品逻辑零改动，未发生「反馈工程师 → 再回归」的循环，2 轮均用于 QA 自修测试与缺陷定位。Round 2 回归为 SOP「反馈修复 → 回归验证」闭环，仅补充 2 项断言级测试，未改动任何产品逻辑。

---

## 4. D. 验收清单逐条核对

**编号口径说明**：任务书称「38 条」。逐条清点 PRD §7 实为 **37 条**（A1~A10 共 10、B1~B10 共 10、C1~C9 共 9、D1~D8 共 8）；DESIGN §10 另有 2 条附加检查项（C10、D9）。本报告按 **39 项**逐条核对，完整覆盖任务书要求的全部条目，不合并、不遗漏。

判定图例：**通过**＝静态/单测已验证；**部分**＝静态符合但观感或运行时行为需 GUI；**需 GUI**＝本环境无法运行 GUI，必须人工截图/交互；**不通过**＝已实证缺陷。

### 4.1 A 组 · 视觉与规格一致性（10 项）

| # | 判定 | 依据 |
| --- | --- | --- |
| A1 | **通过**（视觉观感需 GUI） | `theme.rs` 单测：浅/深 S0<S1<S2≤S3，`chrome=strip=status`；色值与 §5.3 逐项一致 |
| A2 | **通过**（单测） | 深色 S1−S0 明度差实测 **32.9 ≥25**；缩略图条观感需截图 |
| A3 | **通过**（静态）／需 GUI | `viewer.rs:38-45` 1pt `rect_stroke` + `StrokeKind::Inside`，绘制在图片之前不遮挡；四档 DPI 单像素无灰边见 C7 |
| A4 | **通过**（静态+单测） | 常量单测 56/32/8/36·10/12·2·16/16；`ui.rs:69-79` `.exact_size(TOPBAR_H)`、margin 16/10 |
| A5 | **通过**（静态+单测） | `grep divider` 无结果（1×18 竖线已删）；132×36、56×30·7 常量单测；`ui.rs:339-440` |
| A6 | **通过**（静态+单测） | `ITEM_STEP=92`、`80×60·8`、间隙 12；相邻净空 `92−80−2×3 = 6pt` ≥6；环外扩 3/2pt/圆角 11 |
| A7 | **部分**／需 GUI | 状态栏 28、11.5pt、3pt 圆点、绿色短提示已移出（grep 确认）；「字重 500」在 egui 无 weight，用 `RichText::strong()` 近似（R-6/U3），分层可辨需目测 |
| A8 | **通过**（静态+单测） | `STRUCTURE_LINE=1.0`；浅 `#DDE0E5` / 深 `#454A53`；`border == structure_stroke` 单测 |
| A9 | **通过**（单测）／需 GUI | 六态单测保证默认态 `fg=icon`、仅选中态用 accent；「每屏最多 1 处实心强调」属全局构图约束，需截图 |
| A10 | **通过**（单测+静态） | 22 个 Icon 变体 paint 不 panic；20/18/1.6 常量；`Play`/`Pause` 已改 `line()` 线稿；空状态用 `Icon::Image`；线稿观感需截图 |

### 4.2 B 组 · 状态与交互（10 项）

| # | 判定 | 依据 |
| --- | --- | --- |
| B1 | **通过**（单测） | `resolve_state_six_states_are_pairwise_distinct`、`resolve_state_selected_background_differs_from_hover_in_both_themes`、`icon_button_six_state_mapping...`；浅深两主题 × 6 类 WidgetKind 开启态底色 = `accent_soft` ≠ hover = 中性叠色 |
| B2 | **部分**／需 GUI | `visuals::update_keyboard_nav` 鼠标按下清 flag + `ui.rs:238-245` 仅在 flag 且持有焦点时画环；「鼠标点击不出现」运行时行为需人工交互 |
| B3 | **需 GUI** | 6 类开关/单选/复选选中态 ≥3:1 对比度需实际渲染量测；静态仅有色值 |
| B4 | **通过**（单测+静态） | `menu_dimension_constants_match_the_s3_spec`（232/12/32、16/4/8）、`self_drawn_menu_controls_render_in_all_states`；`overlay.rs` 自绘复选/单选、危险项分支 |
| B5 | **部分**／需 GUI | `ui.rs:416-439`：值块 `clicked → Action::Fit`、禁用显示 `—`、`on_hover_text` 保留；点击行为需人工 |
| B6 | **通过**（静态）／需 GUI | `viewer.rs:137-200`：96×96 容器、17/12.5 字阶、132×36 主按钮、`ui.interact` 返回 `open_clicked` → `app::canvas` 打开对话框；点击需人工 |
| **B7** | **通过（修复后回归）** | 错误卡片为画布内 `Area` 浮层、`error_banner` Panel 已删、画布可用高度不变 ✅；删除 `Area::interactable(false)` 后关闭/重试按钮可点（复现测试 `error_card_close_button_is_reachable_by_a_real_click` 现为 `Dismiss`），且新增穿透测试证明无错误 / alpha≤0 时画布点击不被拦截 ✅ → 见 §1.5 |
| B8 | **通过**（单测+静态） | `three_phase_alpha` 三段无硬切、`Motion` 4000ms、`set_notice` 新提示重置计时、`overlay_geometry` toast 右下 12/12 且不感知条显隐 |
| B9 | **部分**／需 GUI | `mask.a()=184(72%)`、`chrome_palette` 单测、`title_total_ms()=3000`、`Esc/F11` 退出逻辑未改 ✅；文字 100% 可读、全屏按钮可点需 GUI |
| B10 | **通过**（单测+静态） | 全部过渡 ≤600ms、`theme_crossfade_ms=0`/`skeleton_ms=0` 装饰默认关闭、`Motion::reduced()` 全 0 且 hold 保持；`overlay.rs:146` 与 `app.rs:1205` 对 `alpha≤0`/`None` 过滤，不下发浮层状态 |

### 4.3 C 组 · 主题 / 语言 / DPI 矩阵（9 项 + 16 组合矩阵）

| # | 判定 | 说明 |
| --- | --- | --- |
| C1 浅+中+1x | **需 GUI** | 静态侧：浅色色板/尺寸/图标单测通过，i18n 零 diff |
| C2 浅+英+1x | **需 GUI** | 英文无截断/重叠需截图；`i18n.rs` 中英资源表逐字未改 |
| C3 深+中+1x | **需 GUI** | 静态侧：深色阶梯单测通过 |
| C4 深+英+1x | **需 GUI** | 同上 |
| C5 跟随系统（中/英/其它） | **需 GUI** | 需多语言系统环境；静态：`i18n.rs` 零 diff + `explicit_language_overrides_system` 单测通过 |
| C6 480px / <720 降级 | **需 GUI** | 静态：`NARROW_SORT_W=720`/`NARROW_COLLAPSE_W=560` + `ui.rs:65` 每帧 `viewport_rect().width()`；`main.rs with_min_inner_size([480,320])` 零 diff |
| C7 1.25x/1.5x/1.75x/2x | **需 GUI** | 静态：`snap*` 四档（1/1.25/1.5/2）单测通过；1.75x 无灰边/圆角/图标清晰度/1px 缝隙需截图 |
| C8 切换主题浮层同步换色 | **需 GUI** | 静态：`theme::palette` 每帧取、`chrome_palette` 每帧、`install` 重装两套 `Visuals`/`Style`、`app.rs:1194` 每帧取色，浮层不缓存 `Color32` |
| C9 切换语言错误卡片即时刷新 | **需 GUI** | 静态：`LocalizedText::render()` 每帧调用（`app.rs:1200`），`i18n.rs`/`errors.rs` 零 diff |
| C10（DESIGN 附加） | **通过**（静态） | `overlay_geometry` 基于 `canvas_rect` 定位，缩略图条显隐不影响 toast |

**16 组合矩阵（浅深 × 中英 × 1x/1.25x/1.5x/2x）——全部需人工 GUI 截图，本环境无法运行 GUI，未伪装通过**：

| 主题 | 语言 | 1x | 1.25x | 1.5x | 2x |
| --- | --- | --- | --- | --- | --- |
| 浅色 | 简体中文 | 需截图 | 需截图 | 需截图 | 需截图 |
| 浅色 | English | 需截图 | 需截图 | 需截图 | 需截图 |
| 深色 | 简体中文 | 需截图 | 需截图 | 需截图 | 需截图 |
| 深色 | English | 需截图 | 需截图 | 需截图 | 需截图 |

> 静态可替代证据：色板浅深单测（6 项）、DPI 取整 4 档单测（5 项）、i18n 资源表零 diff + 4 项 i18n 单测、图标 22 变体绘制单测。**这些不等同于视觉验收通过**。

### 4.4 D 组 · 零回退与质量门禁（8 项 + 1 附加）

| # | 判定 | 依据 |
| --- | --- | --- |
| D1 | **部分**／需 GUI | README 16 键与 `consume_key` 逐字未改（§2.2 #5）、帮助窗口 14 处 `Key::` 沿用既有键；「逐键实测」需 GUI |
| D2 | **需 GUI** | 功能集相关函数 diff 未触及；打开/翻页/缩放/旋转/幻灯片/删除/复制路径/定位/日志/许可证/更新入口需人工回归 |
| D3 | **需 GUI + 重启** | `show_thumbnails` 字段与 `app.ron` 持久化零 diff；显示/隐藏/重启保持需人工 |
| D4 | **需性能实测** | 静态：无新增每帧解码/线程，±3 预取与虚拟滚动算法零 diff；1000/3000 张滚动与内存需实测 |
| D5 | **通过** | `Cargo.toml`/`apps/eframe-app/Cargo.toml`/`Cargo.lock` 空 diff；fmt / clippy -D warnings / check 全绿；`cargo test --workspace` 补测前全绿（补测后因缺陷复现测试变红，见 §1.2） |
| D6 | **通过**（静态+单测） | `crates/iv-infra` 零 diff；`settings::tests` 10 项含 `corrupt_record_falls_back_to_defaults`、`incompatible_version_falls_back_to_defaults`、`missing_fields_use_defaults` 全通过 |
| D7 | **通过**（按本次任务口径） | `design/redesign/*`、`README.md` 零 diff；新增 docs 恰为 2 份（PRD + DESIGN），符合主理人给定的改动集合。注：PRD D7 / DESIGN D7 原文均写「本阶段仅新增一份文档」，属各自阶段内表述；两阶段合流后为 2 份 |
| D8 | **需三端 GUI** | 静态：`titlebar.rs` 仅 `HEIGHT 32→36` 与取色（6 行 diff），`CORNER_RADIUS=8`/拖拽/双击/窗口按钮/缩放把手逻辑未改；`app.rs:1032-1040 clear_color` Linux 全透明保留未改 |
| D9（DESIGN 附加） | **通过** | `i18n.rs`/`errors.rs`/`document.rs`/`cache.rs`/`update.rs`/`crates/*` 全部空 diff |

### 4.5 验收统计

| 判定 | 数量 |
| --- | --- |
| 通过（静态 + 单测） | 22 |
| 部分通过（静态符合 / 需 GUI 确认） | 5 |
| 需人工 GUI 截图或交互 | 11（含 16 组合矩阵按 1 组计） |
| **不通过** | **1（B7）** |
| 合计 | 39 |

---

## 5. 需人工 GUI 复核项（汇总，切勿视为已通过）

1. **16 组合矩阵**：浅/深 × 中/英 × 1x/1.25x/1.5x/2x 全量视觉检查（C1~C4、C7），重点 1pt 线无灰边、圆角无锯齿、分区无 1px 缝隙、英文无截断/重叠。
2. **A 组观感**：A1/A2/A3/A4/A5/A6/A9/A10 的实际渲染量测；尤其「每屏最多 1 处实心强调」的全局构图。
3. **A7/B3**：`RichText::strong()` 近似的字重分层是否可辨；6 类选中态浅深对比度是否 ≥3:1。
4. **B2**：Tab 键出现焦点环、鼠标点击不出现焦点环。
5. **B5/B6**：缩放值块点击适配、空状态主按钮点击打开文件对话框、`—` 禁用态、全部 tooltip。
6. **B7**：错误卡片关闭/重试按钮（**当前判定不可点，需修复后优先复核**）。
7. **B8/B9**：toast 渐隐无硬切且不遮挡状态栏/缩略图条；全屏 @72% 下文字可读、按钮可点、文件名胶囊 3.0s 淡出、`Esc` 恢复。
8. **C5/C6**：跟随系统在中文/英文/其它语言系统下的显示；480px 最小宽度无重叠、<720px 排序降级为图标且功能可达。
9. **C8/C9**：切换主题后所有浮层同步换色；错误卡片显示时切换语言文案即时整体刷新。
10. **D1~D4/D8**：16 键逐条实测、功能集全量回归、缩略图条重启保持、1000/3000 张性能、Windows/macOS/Linux 三端标题栏与透明窗口。
11. **Q5/C7**：80×60 卡片在 2× 屏（需 160 物理像素）依赖 `MAX_SIDE=128` + mipmap/LINEAR 的清晰度；按 Q5 口径不回退尺寸、不改纹理策略，若模糊记为已知限制。
12. **浮层层级**：错误卡片与文件名胶囊同时出现时的避让观感；帮助/更新窗口压在画布浮层之上。

---

## 6. E. 风险复核（独立查证，不采信工程师结论）

| # | 风险点 | 复核方法 | 结论 |
| --- | --- | --- | --- |
| E1 | egui 0.37 `screen_rect()` 已移除，改用 `viewport_rect()`；`Id::new` deprecated 集中于 `stable_id()` 加 `#[allow(deprecated)]` | grep 全 `src/`：`screen_rect()` 仅出现在 `ui.rs:63` 注释；`ui.rs:65`、`titlebar.rs:151` 使用 `viewport_rect()`；`Id::new` 仅 `visuals.rs:313` 一处且带 `#[allow(deprecated)]`；`cargo clippy --workspace --all-targets -- -D warnings` 退出码 0 | ✅ 干净通过 |
| E2 | **`Area::interactable(false)` 下子 `Ui::interact` 是否仍可点击** | ① 查 egui 0.37 源码（`E:/caches/.cargo/.../egui-0.37.0`）：`context.rs:517-522` hit-test 过滤 `is_interactable(layer_id)`；`memory/mod.rs:1241-1242` `is_interactable` 读 `AreaState.interactable`；`area.rs:538` `state.interactable = interactable && !sizing_pass`；`context.rs:1806` `clicked = Some(id) == interact_widgets.clicked`（源自 hits）。② 写最小 headless 行为探针（两帧注册 + 分帧 press/release，规避 sizing pass）实证 | ❌ **不通过**：`interactable(true)` 可点、`interactable(false)` 不可点；`overlay.rs:80` 因此使错误卡片按钮失效，见 §8 |
| E3 | alpha≤0 时不拦截画布点击 | `overlay.rs:146` `alpha.filter(\|v\| *v>0)`；`app.rs:1205-1211` alpha≤0/`None` 不下发浮层状态；`three_phase_alpha` 单测验证 `None` 边界 | ✅ 逻辑通过（但由于 E2 的 layer 本身被排除，画布点击本就不会被浮层拦截——属于「歪打正着」，不能替代 E2 修复） |
| E4 | 错误卡片与文件名胶囊避让 | `visuals::overlay_geometry`：`has_error` 时 `title_top = error.max.y + GROUP_GAP(12)`；单测断言 `!error.intersects(title)` 且 `title.min.y ≥ error.max.y+12` | ✅ 通过 |
| E5 | 帮助/更新窗口压在浮层之上 | `app.rs:1241-1242` 在 `overlay::draw_all` **之后**调用 `ui::help_window` / `ui::update_window`；二者为 `egui::Window`（egui 原生 Foreground 层），画布浮层固定 `Order::Middle` | ✅ 通过（层级约定正确） |
| E6 | 全屏 @72% 与 Linux 透明窗口 `clear_color` 冲突 | `visuals::chrome_palette` 仅替换 `chrome/strip/status` 为 `mask`（不改 `clear_color`）；`app.rs:1032-1040` Linux 返回 `[0,0,0,0]` 全透明，零 diff；`mask.a()=184` 单测 | ✅ 规避正确 |
| E7 | Q6 减少动效默认路径 | `Motion::reduced()` 全部过渡 0、hold 保持 4000/3000；`set_reduced_motion` 默认 false（未新增设置项/i18n 键/线程/定时器）；`crates/*` 与 `i18n.rs` 零 diff；单测通过 | ✅ 通过 |
| E8 | Q5 卡片 80×60 与 MAX_SIDE 128 冲突 | `MAX_SIDE=128`、`MAX_ENTRIES=512`、256MB、2 线程、`mipmap+LINEAR` 全部零 diff；`warm_thumbnails ±3` 未改 | ⚠️ 数值不冲突，但 2× 屏 160 物理像素需 ≤1.25× 放大，**清晰度需 GUI 截图**（按 Q5 记为已知限制，不回退纹理策略） |
| E9 | Z1 toast 4.0s 口径 | `Motion` 160/3520/320 = **4000ms**；`NOTICE_DURATION=4s` 与 `motion.toast_total_ms()` 取 max；文件名 200/2400/400 = **3000ms**；单测通过 | ✅ 与 §9.3 Z1「以 4.0s 为准」一致 |

---

## 7. 源码缺陷详情（已修复 · Round 2）

> **状态：已于 Round 2（2026-11-13）由工程师按方向 a（双 Area 拆分）修复，QA 回归 71/71 全绿（见 §1.5）。以下为缺陷定位与根因留档，供后续审计。**

### 7.1 缺陷描述

| 项 | 内容 |
| --- | --- |
| 缺陷编号 | QA-REDESIGN-001（P0，交互阻断） |
| 源文件 | `apps/eframe-app/src/overlay.rs` |
| 关键行号 | **`overlay.rs:80`** `.interactable(false)`；**`overlay.rs:294`**（关闭按钮 `ui.interact`）；**`overlay.rs:313`**（重试按钮 `ui.interact`）；关联 `overlay.rs:76-82` `Area` 构造、`app.rs:1219-1239` 动作分发 |
| 失败测试 | `overlay::qa_redesign_tests::error_card_close_button_is_reachable_by_a_real_click` |
| 期望 | 点击错误卡片「关闭」按钮返回 `ErrorAction::Dismiss`（PRD §7 B7；DESIGN §8.4「浮层按钮用 `ui.interact`」、§9.1 R-2 明确要规避「关闭/重试点不到」） |
| 实际 | `ErrorAction::None`（按钮完全不响应点击） |
| 错误信息 | `assertion 'left == right' failed: B7：错误卡片「关闭」按钮必须可达（overlay.rs 的 Area 不应吞掉点击）` / `left: None, right: Dismiss` |
| 辅助确证测试 | `overlay::qa_redesign_tests::egui_area_interactable_false_excludes_child_widgets_from_hit_test` **通过**（对照组 `interactable(true)` 可点、`interactable(false)` 不可点），证明注入机制有效、失败归因于产品实现而非测试 |

### 7.2 根因链（egui 0.37 源码证据）

```
overlay.rs:76-82  Area::new(...).order(Order::Middle).interactable(false)
   → area.rs:538          state.interactable = interactable && !sizing_pass  (= false)
   → memory/mod.rs:1241   Areas::is_interactable(layer) = state.interactable
   → context.rs:517-522   begin_pass 构造 hit-test 的 layers 时
                          .filter(|layer_id| self.memory.areas().is_interactable(*layer_id))
                          ⇒ 整个浮层 layer 的 widgets 被排除
   → hit_test.rs         只遍历传入的 layer_order
   → interaction.rs      InteractionSnapshot.clicked 仅来自 hits
   → context.rs:1806     let clicked = Some(id) == viewport.interact_widgets.clicked;
   → create_widget 的 Response::clicked() 恒为 false
```

即：`Area::interactable(false)` 的语义是「整个 layer 点击穿透」，**并非**「仅背景穿透、子控件仍可交互」。因此 `error_card()` 内两处 `ui.interact(..., Sense::click())` 永远不会产生 `clicked()`，`ErrorAction::Dismiss/Retry` 均不可达，错误卡片只能靠「换图成功」被动清除。

### 7.3 修复方向（供工程师/架构师决策，QA 不修改产品逻辑）

需同时满足两个互斥目标：① 错误卡片按钮可点；② 浮层其余区域点击穿透到画布（不破坏缩放/平移/拖拽）。可选方向：
- 把「绘制型浮层」（阴影 / toast / 文件名胶囊）与「交互型错误卡片」拆成两个 `Area`：前者保持 `interactable(false)`，后者 `interactable(true)` 且把 Area 的 rect 严格收缩到卡片区域，并用 `.sense(Sense::hover())` 避免 `move_response` 抢点击；
- 或把错误卡片的 `ui.interact` 移出 `Area`，改在画布 `Ui`（CentralPanel 层）内以绝对 rect 注册，让 hit-test 自然覆盖画布的 `allocate_rect`；
- 无论采用哪种，须保证 `alpha≤0` 或错误不存在时**完全不注册** interact（现有 `overlay.rs:113/146` 与 `app.rs:1205` 的过滤需保留并验证）。

### 7.4 修复后验收建议

1. `cargo test -p image-view`（69/69）；
2. `cargo test --workspace`、`cargo test --workspace --no-default-features`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --all -- --check`、`cargo check -p image-view` 全绿；
3. 人工 GUI 复核 B7：错误出现时点击「关闭」立即消失、可重试时点击「重试」重新加载、点击卡片空白处仍可缩放/拖拽画布。

---

## 8. 智能路由判定与遗留问题

### 8.1 路由判定

> **`NoOne`（修复后回归成功）**

- **首轮定位（Round 1/2 探针）**：断言期望（PRD B7 / DESIGN §8.4）正确，实现给出错误输出；失败测试 `error_card_close_button_is_reachable_by_a_real_click` 经对照语义探针验证为有效，确为**源码 Bug**（QA-REDESIGN-001），初判 `Engineer` 并转交工程师修复。
- **本轮回归（Round 2 修复后）**：工程师按方向 a 修复 `overlay.rs` 双 Area 拆分后，QA 独立重跑 5 项门禁 **5/5 全绿**、`cargo test -p image-view` **71/71**、原复现测试转绿、新增 2 项穿透测试全绿。源码 Bug 已消除，**不再判定 Engineer**，最终路由为 **`NoOne`（成功）**。

> 全程 QA 未自行修改产品逻辑；本轮仅新增 2 项断言级测试（不删除任何既有测试）。

### 8.2 遗留问题（Known Issues）

| # | 问题 | 影响 | 状态 |
| --- | --- | --- | --- |
| K1 | `QA-REDESIGN-001`：`Area::interactable(false)` 导致错误卡片关闭/重试按钮不可点 | ~~B7 不通过；错误无法主动关闭/重试~~ → **已于 Round 2 修复**；双 Area 拆分后复现测试转绿（71/71） |
| K2 | `cargo test --workspace` 等 3 项测试门禁曾返回 101（68/1） | 交付前门禁一度为红 | **已于 Round 2 修复后恢复 5/5 全绿**（见 §1.5.1） |
| K3 | C1~C7 的 16 组合矩阵及 A/B/C/D 组共 20 类视觉/交互项 | 无法在本环境验证 | **需人工 GUI 截图/交互复核**，清单见 §5；本报告未标注为通过 |
| K4 | Q5：80×60 卡片在 2× 屏依赖 128pt 纹理放大 | 可能轻微模糊 | 按 Q5 口径记为已知限制，不回退尺寸与纹理策略；需截图确认 |
| K5 | A7「字重 500」在 egui 无 weight，用 `RichText::strong()` 近似 | 分层是否可辨 | 需目测（U3 已默认此方案） |
| K6 | `thumbs::tests::visible_range_virtualizes_large_lists` 期望值改为由 `ITEM_STEP` 派生 | 断言独立验证力下降 | QA 已补硬编码 92 的独立测试，建议保留两份 |
| K7 | `--no-default-features` 下 4 个 `Key` 变体 dead_code warning | 仅 warning | 已查证为 `errors.rs` 条件编译的既有现象，与本次改动无关；不阻塞 |
| K8 | `main` 分支领先 redesign 一个 `.gitignore` commit | 用 `git diff main` 会误判越界改动 | 已查明；正确基线与结论见 §2.1 |

### 8.3 交付物

- 本报告：`docs/UI-REDESIGN-QA.md`
- 37 项补充自动化测试（含 Round 2 新增 2 项穿透测试）：追加于 `visuals.rs`(17)、`theme.rs`(6)、`icons.rs`(2)、`overlay.rs`(6)、`thumbs.rs`(3)、`ui.rs`(3) 的 `#[cfg(test)] mod qa_redesign_tests`；**未新增文件**
- 未执行任何 git 分支/切换/提交操作；未读写禁用的 `gui-tauri-f66dfece` 与 `E:/private/Rust/image-view` 源码（仅按允许复用 `target` 编译缓存，并只读查阅 `E:/caches/.cargo/.../egui-0.37.0` 依赖源码用于语义查证）

---

## 9. 测试报告（标准格式）

```
# Test Report

## Summary
- Total Tests: 71 (原有 34 + 新增 37) | Passed: 71 | Failed: 0
- 新增测试通过率: 37/37 = 100%；总通过率 100%
- Coverage: 新模块 visuals.rs / overlay.rs / theme.rs / icons.rs / ui.rs / thumbs.rs 的
  公共常量、纯函数、几何、状态矩阵、动效时长、图标绘制均覆盖；本轮新增画布穿透断言覆盖
  「无错误 / alpha≤0 时不创建 error Area、画布点击穿透」；GUI 渲染路径未覆盖（需人工）
- Routing Decision: NoOne（Round 2 修复后回归成功）
- Rounds: 2 / 2（Round 1 定位 P0 → Round 2 修复后回归，完整闭环）

## Failed Tests
- （无；原失败项 `overlay::qa_redesign_tests::error_card_close_button_is_reachable_by_a_real_click`
  在工程师修复 overlay.rs 双 Area 拆分后已转绿：Expected/Actual 均为 `Dismiss`）

## Known Issues
- 仅剩 K3/K4/K5/K6/K7/K8（均非阻塞）：K3 需人工 GUI 复核 16 组合矩阵等 20 类项目；
  K4 Q5 2× 缩略图清晰度；K5 A7 字重 500 近似；K6 visible_range 断言同义反复；
  K7 --no-default-features 既有 dead_code warning；K8 main 分支基线陷阱。
  K1 / K2（P0 缺陷与门禁红）已于 Round 2 关闭。
```
