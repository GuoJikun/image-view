//! 与桌面环境交互的文件操作：回收站与在文件管理器中定位。

use std::path::Path;
use std::process::Command;

#[derive(Debug, thiserror::Error)]
pub enum FileOpError {
    #[error("移到回收站失败：{0}")]
    Trash(#[from] trash::Error),
    #[error("无法打开文件管理器：{0}")]
    Reveal(#[from] std::io::Error),
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
