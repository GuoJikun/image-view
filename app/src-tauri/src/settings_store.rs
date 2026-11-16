//! `settings.ron` 的读取与防抖持久化。
//!
//! 设置文件很小，但翻页/拖拽会连续触发会话更新；这里用后台线程把 400 ms 内的
//! 多次修改合并为一次原子写入，关闭窗口时再同步落盘一次。
//! 读取失败、内容损坏或版本不兼容一律回退默认值，不阻塞启动。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::thread;
use std::time::Duration;

use iv_infra::settings::Settings;

/// 防抖窗口：连按翻页时只写入最终状态。
const DEBOUNCE: Duration = Duration::from_millis(400);

pub struct SettingsStore {
    path: PathBuf,
    state: Mutex<Settings>,
    write_lock: Mutex<()>,
    dirty: AtomicBool,
    tx: Sender<()>,
}

impl SettingsStore {
    /// 读取已有设置并启动后台写入线程。
    pub fn open(path: PathBuf) -> Arc<Self> {
        let settings = read_settings(&path);
        let (tx, rx) = mpsc::channel::<()>();
        let store = Arc::new(Self {
            path,
            state: Mutex::new(settings),
            write_lock: Mutex::new(()),
            dirty: AtomicBool::new(false),
            tx,
        });
        spawn_writer(Arc::downgrade(&store), rx);
        store
    }

    pub fn snapshot(&self) -> Settings {
        lock(&self.state).clone()
    }

    /// 修改设置并安排一次防抖写入。
    pub fn update(&self, apply: impl FnOnce(&mut Settings)) {
        apply(&mut lock(&self.state));
        self.dirty.store(true, Ordering::Relaxed);
        // 接收端只在写入线程退出后才会断开；失败说明应用正在退出。
        let _ = self.tx.send(());
    }

    /// 立即写盘；关闭窗口、退出前调用，确保最后一次修改不丢。
    pub fn flush(&self) {
        if !self.dirty.swap(false, Ordering::Relaxed) {
            return;
        }
        let _guard = lock(&self.write_lock);
        let settings = self.snapshot();
        let Some(text) = settings.to_ron() else {
            return;
        };
        if let Err(error) = write_atomic(&self.path, &text) {
            log::warn!(
                target: "image_view::settings",
                "无法写入设置文件 {}：{error}",
                self.path.display()
            );
            // 下次关闭/退出时再试一次。
            self.dirty.store(true, Ordering::Relaxed);
        }
    }
}

fn spawn_writer(weak: Weak<SettingsStore>, rx: Receiver<()>) {
    let _ = thread::Builder::new()
        .name("iv-settings".to_owned())
        .spawn(move || {
            while rx.recv().is_ok() {
                // 合并防抖窗口内的连续修改。
                while rx.recv_timeout(DEBOUNCE).is_ok() {}
                let Some(store) = weak.upgrade() else {
                    break;
                };
                store.flush();
            }
        });
}

fn read_settings(path: &Path) -> Settings {
    match std::fs::read_to_string(path) {
        Ok(text) => match Settings::from_ron(&text) {
            Some(settings) => settings,
            None => {
                log::warn!(
                    target: "image_view::settings",
                    "设置文件 {} 无法解析，使用默认值",
                    path.display()
                );
                Settings::default()
            }
        },
        Err(error) => {
            if error.kind() != std::io::ErrorKind::NotFound {
                log::warn!(
                    target: "image_view::settings",
                    "无法读取设置文件 {}：{error}",
                    path.display()
                );
            }
            Settings::default()
        }
    }
}

/// 先写临时文件再重命名，避免写入中断留下半个 RON。
fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("ron.tmp");
    std::fs::write(&temp, text)?;
    match std::fs::rename(&temp, path) {
        Ok(()) => Ok(()),
        Err(_) => {
            // 某些文件系统/权限组合不允许覆盖重命名，退回直接写入。
            std::fs::write(path, text)?;
            let _ = std::fs::remove_file(&temp);
            Ok(())
        }
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "image-view-settings-test-{}-{tag}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn update_and_flush_writes_readable_settings() {
        let dir = temp_dir("round-trip");
        let path = dir.join("settings.ron");
        let store = SettingsStore::open(path.clone());

        store.update(|settings| {
            settings.last_dir = Some(dir.clone());
            settings.last_file = Some(dir.join("b.jpg"));
            settings.show_thumbnails = false;
        });
        store.flush();

        let reloaded = SettingsStore::open(path);
        let settings = reloaded.snapshot();
        assert_eq!(settings.last_file, Some(dir.join("b.jpg")));
        assert!(!settings.show_thumbnails);

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn corrupt_file_falls_back_to_defaults() {
        let dir = temp_dir("corrupt");
        let path = dir.join("settings.ron");
        std::fs::write(&path, "{ not valid ron").expect("write fixture");

        let store = SettingsStore::open(path);
        assert_eq!(store.snapshot(), Settings::default());

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }

    #[test]
    fn flush_without_changes_does_not_create_file() {
        let dir = temp_dir("no-write");
        let path = dir.join("settings.ron");
        let store = SettingsStore::open(path.clone());
        store.flush();

        assert!(!path.exists());

        std::fs::remove_dir_all(&dir).expect("cleanup");
    }
}
