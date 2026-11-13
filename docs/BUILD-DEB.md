# 构建 Debian/Ubuntu 安装包（.deb）

| 项目 | 说明 |
| --- | --- |
| 构建主机 | Linux（推荐 WSL2）；Windows 版 cargo-packager 不包含 deb 后端 |
| 打包工具 | cargo-packager 0.11.8 |
| 配置 | `Packager.toml` 的 `[deb]`、`[[fileAssociations]]` 与 `icons` |
| 产物 | `dist/image-view_<version>_<arch>.deb` |

## 为什么不能直接在 Windows 构建

cargo-packager 的 deb 打包模块只在 Linux/BSD 上编译。在 Windows 上执行
`cargo packager --formats deb` 只会得到 `WARN ignoring deb`，不会生成安装包。
此外，deb 内必须是 Linux ELF 二进制，Windows 版 rustc 默认只产出 `.exe`。

## WSL2 一次性准备

在管理员 PowerShell 中确认已安装 WSL2 和 Ubuntu，然后进入 Ubuntu：

```powershell
wsl -d Ubuntu-26.04
```

安装构建依赖和 Linux 版 Rust：

```bash
sudo apt update
sudo apt install -y build-essential pkg-config curl rsync \
  libx11-dev libxcb1-dev libxkbcommon-dev libwayland-dev \
  libgl1-mesa-dev libegl1-mesa-dev libfontconfig1-dev

curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
source "$HOME/.cargo/env"
cargo install cargo-packager --version 0.11.8 --locked
```

用 `which cargo` 确认是 `/home/.../.cargo/bin/cargo`，而不是 `/mnt/c/.../cargo.exe`。

## 构建

建议把仓库复制到 Linux 文件系统再构建，避免 WSL 跨 `/mnt/e` 读写缓慢，也避免
Windows/Linux 共用 `target/`：

```bash
mkdir -p ~/src
rsync -a --exclude target /mnt/e/private/Rust/image-view/ ~/src/image-view/
cd ~/src/image-view
bash scripts/build-deb.sh
```

脚本会执行：

```bash
cargo packager --release -p image-view -c Packager.toml --formats deb
```

产物：

```text
dist/image-view_1.3.0_amd64.deb
```

复制回 Windows：

```bash
cp dist/image-view_*.deb /mnt/e/private/Rust/image-view/dist/
```

## 检查与安装

```bash
dpkg-deb --info dist/image-view_1.3.0_amd64.deb
dpkg-deb --contents dist/image-view_1.3.0_amd64.deb
sudo apt install ./dist/image-view_1.3.0_amd64.deb
image-view
```

包内应包含：

- `/usr/bin/image-view`
- `/usr/share/applications/image-view.desktop`
- `/usr/share/icons/hicolor/64x64/apps/image-view.png`
- `/usr/share/icons/hicolor/256x256/apps/image-view.png`
- `/usr/share/icons/hicolor/512x512/apps/image-view.png`
- `/usr/share/icons/hicolor/1024x1024/apps/image-view.png`

## 安装失败排查

### 提示缺少 `libxkbcommon-x11-0`

用 `dpkg -i image-view_*.deb` 或部分图形安装器安装时，dpkg 只解包、不会下载依赖，
`image-view` 会停在 `iU`（已解包、未配置）状态。请改用 apt 安装并补齐依赖：

```bash
sudo apt update
sudo apt --fix-broken install
sudo apt install ./dist/image-view_1.3.0_amd64.deb
```

`libxkbcommon-x11-0` 会连带安装 `libxcb-xkb1`，两者都在 Ubuntu 22.04 / 24.04 / 26.04
的 `main` 源中。安装后用下面的命令确认状态已从 `iU` 变为 `ii`：

```bash
dpkg -l image-view
image-view
```

如果 `apt` 仍提示依赖不可安装，先确认软件源和网络正常：

```bash
apt-cache policy libxkbcommon-x11-0
sudo apt update
```

## 配置说明

`Packager.toml` 中与 deb 有关的字段：

- `authors`：生成 Debian control 的 `Maintainer` 字段，必须是非空真实联系人。
- `homepage`：生成 `Homepage` 字段。
- `icons`：deb 只消费 PNG；`icon.png`（64×64）与 `icon-256.png`、`icon-512.png`、
  `icon-512@2x.png`（1024×1024）会安装到对应 hicolor 目录。NSIS 仍优先使用
  `icon.ico`。`@2x` 后缀是 cargo-packager 判断 Retina 密度的约定，macOS 打包依赖它。
- `[deb]`：包名、section、priority 与运行时依赖。cargo-packager 没有
  `$auto` 依赖模式，依赖需要手动维护。
- `[[fileAssociations]]`：在 Linux 上生成 `.desktop` 的 `MimeType=...`，在 macOS 上生成
  `CFBundleDocumentTypes`；Windows 安装器继续使用
  `installer/nsis/image-view.nsi` 里的硬编码注册。

## 兼容性

deb 的最低 glibc 版本取决于构建主机。在 Ubuntu 26.04 上构建的包通常无法安装到
Ubuntu 22.04 / Debian 12；正式发布建议在较旧的发行版（例如 Ubuntu 22.04）或对应
容器中构建。

Linux 端的自动更新目前不可用（`UpdateError::Unsupported`），请通过
`apt upgrade` 或重新安装新版本升级。
