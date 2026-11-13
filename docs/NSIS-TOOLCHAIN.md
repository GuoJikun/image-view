# NSIS 工具链预置（cargo-packager）

| 项目 | 内容 |
| --- | --- |
| 适用版本 | cargo-packager v0.11.8 |
| 目标目录 | `%LOCALAPPDATA%\.cargo-packager` |
| 工具链版本 | NSIS 3.09（`makensis v3.09`） |
| 最后验证 | 2026-11-10 |

## 1. 为什么需要手工预置

`cargo packager` 首次打包时会从 GitHub Release 下载 NSIS 工具链。受网络限制时该下载可能长时间停在 `release-assets.githubusercontent.com`，最终超时失败。

更关键的是它的校验行为：**启动打包时若目标目录缺少清单中的任意一个文件，会删除整个 `NSIS` 目录并重新下载**，日志为：

```text
WARN NSIS directory is missing some files. Recreating it...
ERROR https://github.com/tauri-apps/binary-releases/.../nsis-3.09.zip: Connection Failed
```

因此“下载慢”往往表现为“目录被反复重建、进度始终为零”。预置必须**一次放全全部文件**。

## 2. 目录结构

```text
%LOCALAPPDATA%\.cargo-packager\
├── NSIS\
│   ├── Bin\makensis.exe                          # 实际调用的编译器
│   ├── Stubs\lzma-x86-unicode
│   ├── Stubs\lzma_solid-x86-unicode
│   ├── Plugins\x86-unicode\ApplicationID.dll     # 快捷方式身份（ApplicationID::Set）
│   ├── Plugins\x86-unicode\nsis_tauri_utils.dll  # 进程检测/结束、SemverCompare
│   ├── Include\MUI2.nsh
│   ├── Include\FileFunc.nsh
│   ├── Include\x64.nsh
│   ├── Include\nsDialogs.nsh
│   ├── Include\WinMessages.nsh
│   └── ...（NSIS 3.09 发行版其余文件）
└── NSIS-ApplicationID.zip                        # 注意：父目录，不在 NSIS 内
```

上表是 cargo-packager 的完整性检查清单，缺一项即触发整目录重建。

## 3. 预置步骤

准备三份原始文件：

| 文件 | 来源 | 参考大小 |
| --- | --- | --- |
| `nsis-3.09.zip` | NSIS 官方发行包（含 441 个条目） | 2,354,828 字节 |
| `appid.zip` | NSIS ApplicationID 插件包（含 `ReleaseUnicode/ApplicationID.dll`） | 2,802,408 字节 |
| `tauri_utils.dll` | `nsis_tauri_utils` 插件（PE 文件，来自 cargo-packager 发行依赖） | 1,188,352 字节 |

```powershell
$src  = "$env:TEMP\iv-nsis"
$root = "$env:LOCALAPPDATA\.cargo-packager"
$dest = Join-Path $root 'NSIS'

New-Item -ItemType Directory -Force -Path $dest | Out-Null
# NSIS 3.09 发行包解压后顶层目录为 nsis-3.09
& 7z x "$src\nsis.zip" "-o$src\extract" -y
Copy-Item "$src\extract\nsis-3.09\*" $dest -Recurse -Force

& 7z x "$src\appid.zip" "-o$src\appid-extract" -y
New-Item -ItemType Directory -Force -Path "$dest\Plugins\x86-unicode" | Out-Null
Copy-Item "$src\appid-extract\ReleaseUnicode\ApplicationID.dll" "$dest\Plugins\x86-unicode\" -Force
Copy-Item "$src\tauri_utils.dll" "$dest\Plugins\x86-unicode\nsis_tauri_utils.dll" -Force
Copy-Item "$src\appid.zip" "$root\NSIS-ApplicationID.zip" -Force
```

## 4. 校验

```powershell
& "$env:LOCALAPPDATA\.cargo-packager\NSIS\Bin\makensis.exe" /VERSION   # 期望输出 v3.09
cargo packager --release -p image-view -c Packager.toml                # 不应出现下载请求
```

打包成功的日志形如：

```text
INFO Running makensis.exe to produce dist\image-view_1.0.0_x64-setup.exe
INFO Finished packaging 1 package
```

## 5. 网络受限时的替代路径

若确实需要让 cargo-packager 自行下载，可通过环境变量指定代理（`reqwest` 读取 `HTTPS_PROXY`）：

```bash
HTTPS_PROXY=http://127.0.0.1:10808 cargo packager --release -p image-view
```

代理需支持 HTTP CONNECT；纯 SOCKS5 端口不一定被 `reqwest` 接受。

## 6. GitHub Actions

`.github/workflows/release.yml` 在 Windows runner 上使用 `scripts\build-release.ps1 -AllowNsisDownload`：

- 首次运行由 cargo-packager 自行下载 NSIS 3.09 及插件到 `%LOCALAPPDATA%\.cargo-packager`，后续通过 `actions/cache` 复用该目录。
- 工作流仅在推送 `v*` 标签时触发，并校验标签与 `Cargo.toml` / `Packager.toml` 的版本一致。
- 产物为安装包、`update.json` 与 `SHA256SUMS.txt`，同时上传为工作流 artifact 并附加到 GitHub Release。

## 7. 维护约定

- 不要单独删除或替换 `NSIS` 目录内的文件；升级 NSIS 版本时应整目录替换，并同步更新本文件的清单。
- `installer/nsis/image-view.nsi` 由项目维护，升级 cargo-packager 后需对照其默认模板检查占位符是否新增（例如 `{{...}}` 变量、`{{#each}}` 段落）。
- 工具链目录不进仓库；新环境按本文件第 3 节重建。
