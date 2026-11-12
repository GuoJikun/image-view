# 构建 macOS 安装包（.dmg）

| 项目 | 说明 |
| --- | --- |
| 构建主机 | Apple Silicon Mac（arm64）；仅支持 arm64，不提供 Intel 或 Universal 版本 |
| 系统要求 | macOS 11+、Xcode Command Line Tools |
| 打包工具 | cargo-packager 0.11.8 |
| 配置 | `Packager.toml` 的 `[macos]`、`[dmg]`、`icons` 与 `[[fileAssociations]]` |
| 产物 | `dist/image-view_<version>_aarch64.dmg` |

## 为什么只支持 arm64

项目只发布 Apple Silicon 版本。`scripts/build-macos.sh` 会在以下情况直接退出：

- 构建主机不是 macOS；
- `uname -m` 不是 `arm64`；
- 当前 shell 运行在 Rosetta 2 下；
- Rust 工具链的 host 不是 `aarch64-apple-darwin`。

制作 Universal 包需要额外安装 x86_64 目标、分别编译再用 `lipo` 合并，目前不在维护范围内。
`[macos] minimumSystemVersion = "11.0"` 与 `aarch64-apple-darwin` 的默认部署目标一致，
也是第一代 Apple Silicon Mac 的起始系统版本。

## 一次性准备

安装 Xcode 命令行工具与原生 Rust 工具链：

```bash
xcode-select --install
rustup toolchain install stable-aarch64-apple-darwin
rustup default stable-aarch64-apple-darwin
```

确认工具链和架构：

```bash
uname -m                                # 应为 arm64
rustc -vV | grep '^host:'               # 应为 host: aarch64-apple-darwin
```

安装 cargo-packager：

```bash
cargo install cargo-packager --version 0.11.8 --locked
```

首次构建 dmg 时，cargo-packager 会联网下载 `create-dmg` 脚本到
`~/Library/Caches/.cargo-packager/DMG/script/create-dmg`；离线环境需要提前准备好该工具。

如果系统使用 SOCKS 代理，cargo-packager 内置的下载器可能报：

```text
ERROR https://raw.githubusercontent.com/create-dmg/...: Connection Failed: Connect error: SOCKS feature disabled.
```

这是因为它的 HTTP 客户端没有启用 SOCKS 支持。用 `curl`（会走系统代理）手动把同一份脚本
放到缓存目录即可，之后打包会直接复用、不再下载：

```bash
mkdir -p ~/Library/Caches/.cargo-packager/DMG/script
curl -fL --retry 5 -C - \
  -o ~/Library/Caches/.cargo-packager/DMG/script/create-dmg \
  https://raw.githubusercontent.com/create-dmg/create-dmg/28867ba3563ddef62f55dcf130677103b4296c42/create-dmg
chmod 764 ~/Library/Caches/.cargo-packager/DMG/script/create-dmg
```

## 构建

在仓库根目录运行：

```bash
bash scripts/build-macos.sh
```

脚本会依次检查系统、架构、Rust host、cargo-packager 与 `hdiutil`，并在构建前把
`MACOSX_DEPLOYMENT_TARGET` 固定为 `11.0`（与 `LSMinimumSystemVersion` 一致），然后执行：

```bash
cargo packager --release -c Packager.toml --formats dmg
```

`beforePackagingCommand` 会先执行 `cargo build --release --locked`。打包完成后脚本用
`lipo -archs target/release/image-view` 确认二进制只包含 `arm64`，避免 Rosetta 工具链
产出 x86_64 包。

产物：

```text
dist/image-view.app
dist/image-view_1.3.0_aarch64.dmg
```

`.app` 是 dmg 的中间产物，可以一起保留或删除。

## 安装与检查

```bash
hdiutil attach dist/image-view_1.3.0_aarch64.dmg
cp -R /Volumes/image-view/image-view.app /Applications/
hdiutil detach /Volumes/image-view
open -a image-view
```

本地检查：

```bash
file target/release/image-view           # Mach-O 64-bit executable arm64
lipo -archs target/release/image-view    # arm64
plutil -p /Applications/image-view.app/Contents/Info.plist
```

`Info.plist` 中应包含：

- `CFBundleIdentifier = io.github.guojikun.image-view`
- `LSMinimumSystemVersion = 11.0`
- `LSApplicationCategoryType = public.app-category.photography`
- 各图片扩展名的 `CFBundleDocumentTypes`（角色为 `Viewer`）

未签名时首次打开会被 Gatekeeper 拦截，在 Finder 中右键应用选择“打开”即可；
也可以在确认来源可信后清除隔离属性：

```bash
xattr -dr com.apple.quarantine /Applications/image-view.app
```

## 配置说明

`Packager.toml` 中与 macOS 有关的字段：

- `[macos] minimumSystemVersion`：写入 `LSMinimumSystemVersion`。
- `[macos] signingIdentity`：默认注释掉，保持本地构建无需证书；取消注释并填入
  “Developer ID Application: …” 后才会签名和触发公证。
- `[dmg]`：设置 dmg 打开后的 Finder 窗口尺寸，以及应用图标和“应用程序”快捷方式的位置。
  如需背景图，可增加 `background = "assets/dmg-background.png"`。
- `icons`：cargo-packager 会把其中的 PNG（64/256/512/1024）合成为 `.app` 内的 `.icns`；
  NSIS 继续使用 `icon.ico`。deb 也会把这些 PNG 安装到对应的 hicolor 目录。macOS 端按
  文件名后缀判断像素密度：`icon-512@2x.png`（1024×1024）表示 512 pt 的 Retina 图标；
  ICNS 没有 1024×1024 的 1x 类型，所以不能命名为 `icon-1024.png`，否则打包会报
  `ERROR No matching IconType`。
- `resources`：`README.md` 与 `CHANGELOG.md` 复制到 `.app/Contents/Resources`。
- `[[fileAssociations]]`：生成 `CFBundleDocumentTypes`，`role = "viewer"` 表示只读查看；
  同一份配置在 Linux 上生成 `.desktop` 的 MIME 列表。
- 顶层 `formats` 仍为 `nsis`（Windows 默认）；macOS 构建通过 `--formats dmg` 覆盖。

## 签名与公证（可选）

默认构建不签名。正式分发建议准备 Apple Developer ID：

1. 把 “Developer ID Application” 证书安装到登录钥匙串；
2. 在 `Packager.toml` 的 `[macos]` 中取消注释并填写 `signingIdentity`；
3. 提供公证凭据（任选一种）：

   ```bash
   export APPLE_ID="you@example.com"
   export APPLE_PASSWORD="app-specific-password"
   export APPLE_TEAM_ID="TEAMID"
   ```

   或使用 App Store Connect API Key：

   ```bash
   export APPLE_API_KEY="KEYID"
   export APPLE_API_ISSUER="ISSUER-UUID"
   export APPLE_API_KEY_PATH="$HOME/private_keys/AuthKey_KEYID.p8"
   ```

4. 重新运行 `bash scripts/build-macos.sh`。

设置 `signingIdentity` 后，cargo-packager 会用 hardened runtime 签名 `.app`、签名 dmg，
并在存在公证凭据时通过 `notarytool` 提交 `.app`、成功后自动 staple；凭据缺失时只警告并
跳过公证。证书与密码不应写进仓库；CI 可使用 `APPLE_CERTIFICATE`
（base64 编码的 p12）与 `APPLE_CERTIFICATE_PASSWORD`。

验证签名与公证：

```bash
codesign -dv --verbose=4 /Applications/image-view.app
spctl -a -vv /Applications/image-view.app
xcrun stapler validate /Applications/image-view.app
```

## 兼容性与限制

- 只提供 arm64、macOS 11+ 的构建；Intel Mac 与 Universal 包不在支持范围内。
- 文件关联已写入 `CFBundleDocumentTypes`，但 eframe/winit 尚未处理 macOS 的
  `application:openURLs:` 事件：从 Finder 双击或“打开方式”目前只会启动应用，不会自动
  载入图片。可先用 `image-view <文件>`，或 `open -n -a image-view --args <文件>`。
- 渲染后端仍是 eframe 的 glow（OpenGL）；macOS 可以运行，但 OpenGL 已被 Apple 标记为弃用。
- macOS 端自动更新不可用（`UpdateError::Unsupported`），升级时请下载新 dmg 覆盖
  “应用程序”中的旧版本。
- 当前没有 macOS CI 任务，dmg 需在本地 Apple Silicon Mac 上构建。
