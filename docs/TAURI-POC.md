# Tauri 2 + Vue 3 前端 POC

本文件记录 `docs/03-tauri-vue-rust-architecture.md` 方案 C 的实现状态、IPC 边界、验收结果与已知缺口。

代码位于 `app/`，全部 Tauri 相关工作保留在 `gui-tauri` 分支，`main` 上的 eframe 应用不受影响。

## 1. 目录结构

```text
app/
├── package.json              # 依赖与脚本（pnpm：dev / build / tauri）
├── index.html                # 前端入口
├── vite.config.ts            # Vite 配置（固定端口 1420）
├── src/                      # Vue 3 + TypeScript + Vite + Pinia
│   ├── services/ipc.ts       # 前端唯一的 Tauri 边界
│   ├── stores/viewer.ts      # 只保存 ID/DTO/视图参数
│   ├── components/           # 工具栏、画布、元数据、缩略图、状态栏
│   └── types/ipc.ts
└── src-tauri/
    ├── Cargo.toml            # package: image-view-tauri
    ├── build.rs
    ├── tauri.conf.json       # 窗口、CSP、beforeDev/Build、bundle
    ├── capabilities/
    │   └── default.json      # 仅 core:default
    ├── icons/
    └── src/
        ├── main.rs           # 桌面入口，调用 lib::run
        ├── lib.rs            # Builder 装配、单实例、命令注册
        ├── dto.rs            # Command/Event DTO 与稳定错误码
        ├── state.rs          # 目录导航、ImageId 授权表、用例编排
        ├── preview.rs        # 预览解码线程、latest-wins、PNG 缓存
        ├── thumbnails.rs     # 缩略图解码与内存 LRU
        ├── protocol.rs       # iviewer:// 资源协议
        └── commands/
            ├── image.rs
            ├── file.rs
            ├── task.rs
            └── update.rs
```

`src-tauri` 只做适配与装配：目录扫描与导航复用 `iv-application::FolderNav`，解码复用 `iv-image-decode`，缩略图缓存复用 `iv-cache::LruCache`，文件管理器定位复用 `iv-platform`，日志复用 `iv-infra::logging`。Core/Application 不依赖 Tauri 或 WebView。

## 2. 运行与构建

```powershell
cd app
pnpm install
pnpm tauri dev              # tauri dev，自动启动 Vite 开发服务器
pnpm tauri build            # tauri build（release + 安装包）
```

仅验证编译、不生成安装包：

```powershell
pnpm tauri build --no-bundle
```

打包需要下载 WiX 与 NSIS 工具链。若 GitHub 下载超时，先设置代理环境变量（Tauri 的下载器读取 `HTTP_PROXY` / `HTTPS_PROXY` / `ALL_PROXY`）：

```powershell
$env:HTTPS_PROXY = 'http://127.0.0.1:10808'
$env:HTTP_PROXY = 'http://127.0.0.1:10808'
pnpm tauri build
```

前端单独构建：

```powershell
cd app
pnpm build                  # vue-tsc --noEmit && vite build
```

Rust 侧检查：

```powershell
cargo fmt --all --check
cargo clippy -p image-view-tauri --all-targets -- -D warnings
cargo test -p image-view-tauri
```

## 3. IPC 边界

### 3.1 Commands（短请求/响应）

| Command | 参数 | 返回 | 说明 |
| --- | --- | --- | --- |
| `open_path` | `path: String` | `DirectorySnapshot` | 规范化路径后打开文件或目录；目录取第一张，文件扫描同目录并定位 |
| `pick_image` | — | `String \| null` | 原生文件对话框，`rfd` 调用放在 blocking 线程池 |
| `navigate` | `delta: i32` | `DirectorySnapshot \| null` | 上一张/下一张，不循环 |
| `select_image` | `index: usize` | `DirectorySnapshot \| null` | 点击缩略图跳转 |
| `current_snapshot` | — | `DirectorySnapshot` | 当前目录快照 |
| `request_preview` | `image_id: String` | `PreviewRequest` | 只接受目录扫描时登记的 ImageId |
| `set_sort` | `order: "name" \| "modified"` | `DirectorySnapshot` | 改变排序并重建列表 |
| `startup_path` | — | `String \| null` | 命令行启动参数 |
| `close_image` | — | — | 取消在途预览并清空预览缓存 |
| `reveal_image` | `image_id: String` | — | 在系统文件管理器中定位 |
| `cancel_preview` | — | `u64` | 推进请求代号，使在途结果全部过期 |

Command 只做参数校验、调用用例与 DTO 转换，没有 `execute_action(action, params)` 式万能接口。

### 3.2 Events（异步状态）

| Event | Payload | 说明 |
| --- | --- | --- |
| `image:preview-ready` | `PreviewReady` | 含 `requestId`、`imageId`、`revision`、尺寸/元数据；不含像素 |
| `image:preview-failed` | `PreviewFailed` | 含 `requestId`、`imageId`、稳定错误码与消息 |
| `app:open-file` | `String` | 单实例把第二个实例的文件参数转交给已运行窗口 |

前端在 `stores/viewer.ts` 中按 `requestId` 与当前 `imageId` 双重过滤，旧请求不会覆盖新图。

### 3.3 Custom Protocol（图片资源）

```text
iviewer://preview/{image_id}/{revision}
iviewer://thumbnail/{image_id}/{size}/{revision}
```

Windows 上 WebView2 实际使用 `http://iviewer.localhost/...`，macOS/Linux 使用 `iviewer://localhost/...`；`services/ipc.ts` 根据 UA 选择，协议 handler 只看 URI path，因此两种形式共用同一套路由。

- URL 只包含后端生成的 UUID `ImageId` 与 revision，不包含文件路径。
- 协议层用 `Uuid::parse_str` 校验 ID，再通过 `AppState` 的授权表查找路径，无法拼接任意路径读取文件。
- 预览像素保存在 Rust 侧，协议首次访问时惰性编码为 PNG 并缓存；编码在独立线程执行，不阻塞 WebView 请求线程。
- 响应头：`Content-Type: image/png`、`Cache-Control: public, max-age=31536000, immutable`、`X-Content-Type-Options: nosniff`。
- 非 GET 返回 405；未知资源 404；缩略图尺寸限制在 32–512。

### 3.4 前端状态

Pinia 只保存 `DirectorySnapshot`、`PreviewReady` 元数据、视图参数（缩放/平移/旋转/翻转/适配模式）、面板显隐、请求代号与错误摘要。原始 RGBA、Base64、文件句柄均不进入前端状态。

## 4. 关键流程

### 4.1 打开与切换

1. 前端调用 `open_path`，Rust 规范化路径、扫描目录、为每个文件生成 UUID，返回轻量快照。
2. 前端调用 `request_preview(imageId)`；Rust 递增全局 `request_id` 与每张图的 `revision`，把任务投递到专用解码线程。
3. 解码线程取队列中最新的任务，丢弃更旧的排队请求；解码前后各检查一次 `request_id`，过期结果既不写缓存也不发事件。
4. 完成事件只携带元数据与资源句柄；前端用 `imageId + revision` 拼出 `iviewer://` URL，由 `<img>` 加载，Canvas 再绘制。
5. 快速连按方向键时，只有最新请求能更新当前图片。

### 4.2 缩放与高 DPI

- `100%` 定义为 1 图像像素 = 1 物理屏幕像素。
- Canvas 的 backing store 按 `devicePixelRatio` 放大，绘制时 `setTransform(dpr, ...)`，因此 CSS 尺寸与资源尺寸分离。
- 适配窗口按物理像素计算上限，且不会把小图放大到超过 100%。
- 滚轮以光标为锚点缩放，拖拽平移；平移范围按旋转后的显示尺寸钳制。
- 旋转/翻转只作用于显示，不写回文件，切换图片后重置。

### 4.3 单实例与文件打开

- `tauri-plugin-single-instance` 必须最先注册；第二个实例把参数转发给已有窗口并聚焦。
- 启动参数与转发参数都要求路径真实存在，忽略 `-` 开头的选项。
- 前端在 `initialize()` 中订阅 `app:open-file`，收到后走与命令行相同的 `openPath` 流程。
- 拖拽由 Tauri 的 `onDragDropEvent` 处理，取第一个路径。

## 5. 安全与能力

- `capabilities/default.json` 只声明 `core:default`，没有文件系统、Shell、HTTP 等插件权限。
- 所有业务能力通过显式 Command 暴露；前端不能直接调用文件系统。
- 路径在 Rust 侧 `canonicalize`，并检查存在且为文件/目录。
- ImageId 必须是 UUID 且存在于当前授权表；前端传入任意字符串会被拒绝。
- 排序参数使用白名单解析，未知值返回 `invalid_argument`。
- CSP：`default-src 'self'`，仅允许 `ipc:`/`http://ipc.localhost` 与 dev server 的 WebSocket，图片源额外允许 `http://iviewer.localhost` 与 `iviewer:`。
- 前端不把文件路径拼进资源 URL；路径只出现在元数据展示中。

## 6. POC 验收状态

| 验收项（docs/03 §12） | 状态 | 说明 |
| --- | --- | --- |
| WebView 运行时与打包策略 | Windows 已验证 | `npm run build` 生成 MSI（6.32 MiB）与 NSIS 安装包（4.43 MiB）；macOS/Linux WebView 尚未验证 |
| 预览协议性能与缓存行为 | 部分验证 | PNG 惰性编码 + 4 张预览 LRU + 512 张缩略图 LRU；缺少真实大量图片切换的基准数据 |
| 大图不经 Base64/JSON | 已验证（代码路径） | 像素只经 `iviewer://` 二进制响应传输；事件/DTO 不含像素 |
| 快速切换旧请求不覆盖新图 | 已验证（单元测试 + 代码路径） | 全局 `request_id` + 每图 `revision`；队列只保留最新任务 |
| 高 DPI Canvas 缩放/旋转/拖动 | 部分验证 | DPR backing store 与视图数学已实现；需在 125%/150%/200% 缩放下手工确认 |
| 文件打开事件、单实例 | 部分验证 | 插件与 `app:open-file` 已接通；需实际启动两个实例验证 |
| capabilities 最小化后流程正常 | 部分验证 | 仅 `core:default`，命令注册完整；需运行窗口逐项点验 |

已完成的自动化检查：

```text
cargo fmt --all --check                         通过
cargo clippy -p image-view-tauri --all-targets 通过（-D warnings）
cargo test --workspace                          通过
cargo test -p image-view-tauri                  9 个单元测试通过
frontend: npm run build                         vue-tsc + vite 通过
npm run build                                   MSI + NSIS 安装包生成成功
release exe 冒烟启动                            带图片参数运行 10s，进程存活并记录启动路径
```

产物：

```text
target/release/image-view-tauri.exe
target/release/bundle/msi/image-view-tauri_1.3.0_x64_en-US.msi      (6.32 MiB)
target/release/bundle/nsis/image-view-tauri_1.3.0_x64-setup.exe     (4.43 MiB)
```

仍需手工验证：

1. `npm run dev` 启动窗口，确认协议图片能加载、缩略图条正常。
2. 快速连按 `←`/`→`，确认最终只显示最后一张。
3. 在 Windows 显示缩放 125%/150%/200% 下检查「适配」「100%」、旋转与拖拽。
4. 从资源管理器/命令行再次打开文件，确认单实例转发并聚焦。
5. 运行 `npm run build` 生成安装包，安装后检查 WebView2 依赖与文件关联（当前未注册文件关联）。

## 7. 已知缺口

- 缩略图只有内存 LRU，没有磁盘缓存；进程重启后需重新解码。
- 缩略图缓存键包含路径、修改时间与尺寸，但协议 URL 中的 revision 未参与缓存键；文件在运行期间被替换时，同一尺寸可能命中旧缩略图。
- 动图只显示第一帧；EXIF 方向与 ICC 转换由 `iv-image-decode` 现有路径处理，与 eframe 版一致。
- 预览缓存最多 4 张，未做相邻图片预加载；切换大图时仍可能看到短暂加载态。
- 预览调度（request_id/latest-wins、PNG 缓存）目前位于 Tauri adapter 的 `preview.rs`，尚未上移到 `iv-application` 的用例层；目录导航与解码已复用共享 crate。
- 未实现设置持久化、窗口状态恢复、主题切换、i18n、删除/重命名、幻灯片、自动更新、文件关联注册。
- 前端界面目前只有简体中文。
- macOS/Linux WebView 行为未在本机验证。
- 安装包未做代码签名；Windows 文件关联尚未注册。
