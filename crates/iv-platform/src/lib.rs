//! 与桌面环境交互：回收站、文件管理器定位与系统语言。

use std::path::Path;
use std::process::Command;

#[derive(Debug)]
pub enum FileOpError {
    Trash(trash::Error),
    Reveal(std::io::Error),
    Open(std::io::Error),
}

impl std::fmt::Display for FileOpError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 平台层只提供稳定的英文描述；界面文案由 GUI 按错误类型本地化。
        match self {
            Self::Trash(error) => write!(formatter, "failed to move to trash: {error}"),
            Self::Reveal(error) => write!(formatter, "failed to open file manager: {error}"),
            Self::Open(error) => write!(formatter, "failed to open file: {error}"),
        }
    }
}

impl std::error::Error for FileOpError {}

impl From<trash::Error> for FileOpError {
    fn from(error: trash::Error) -> Self {
        Self::Trash(error)
    }
}

impl From<std::io::Error> for FileOpError {
    fn from(error: std::io::Error) -> Self {
        Self::Reveal(error)
    }
}

/// 把文件移到系统回收站（可恢复）。
pub fn delete_to_trash(path: &Path) -> Result<(), FileOpError> {
    trash::delete(path)?;
    Ok(())
}

/// 在系统文件管理器中选中该文件。
pub fn reveal_in_file_manager(path: &Path) -> Result<(), FileOpError> {
    reveal_command(path).spawn()?;
    Ok(())
}

/// 在系统文件管理器中打开该目录。
pub fn open_folder(path: &Path) -> Result<(), FileOpError> {
    open_path_command(path).spawn()?;
    Ok(())
}

/// 用系统默认程序打开文件。
pub fn open_file(path: &Path) -> Result<(), FileOpError> {
    open_path_command(path).spawn().map_err(FileOpError::Open)?;
    Ok(())
}

/// 系统界面语言的 BCP-47 标签（如 `zh-CN`、`en-US`）。
///
/// 前端起来之前（原生菜单首帧、启动日志）只能靠这里判断语言；
/// 读取失败时返回 `None`，调用方按英文回退。
pub fn system_locale() -> Option<String> {
    sys_locale::get_locale()
}

#[cfg(target_os = "windows")]
fn open_path_command(path: &Path) -> Command {
    let mut command = Command::new("explorer");
    command.arg(path);
    command
}

#[cfg(target_os = "macos")]
fn open_path_command(path: &Path) -> Command {
    let mut command = Command::new("open");
    command.arg(path);
    command
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn open_path_command(path: &Path) -> Command {
    let mut command = Command::new("xdg-open");
    command.arg(path);
    command
}

#[cfg(target_os = "windows")]
fn reveal_command(path: &Path) -> Command {
    let mut command = Command::new("explorer");
    // 路径单独传参，由 Windows 负责引用；explorer 成功时也可能返回非 0，因此只看能否启动。
    command.arg("/select,").arg(path);
    command
}

#[cfg(target_os = "macos")]
fn reveal_command(path: &Path) -> Command {
    let mut command = Command::new("open");
    command.arg("-R").arg(path);
    command
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn reveal_command(path: &Path) -> Command {
    let mut command = Command::new("xdg-open");
    command.arg(path.parent().unwrap_or(path));
    command
}
