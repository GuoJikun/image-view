//! 与桌面环境交互的文件操作：回收站与在文件管理器中定位。

use std::path::Path;
use std::process::Command;

#[derive(Debug)]
pub enum FileOpError {
    Trash(trash::Error),
    Reveal(std::io::Error),
}

impl std::fmt::Display for FileOpError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // 平台层只提供稳定的英文描述；界面文案由 GUI 按错误类型本地化。
        match self {
            Self::Trash(error) => write!(formatter, "failed to move to trash: {error}"),
            Self::Reveal(error) => write!(formatter, "failed to open file manager: {error}"),
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
    open_folder_command(path).spawn()?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn open_folder_command(path: &Path) -> Command {
    let mut command = Command::new("explorer");
    command.arg(path);
    command
}

#[cfg(target_os = "macos")]
fn open_folder_command(path: &Path) -> Command {
    let mut command = Command::new("open");
    command.arg(path);
    command
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn open_folder_command(path: &Path) -> Command {
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
