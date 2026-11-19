//! 后台索引线程：扫描目录并把条目写入 SQLite。
//!
//! `Connection` 非 `Sync`，由本线程独占可写连接；查询端用另一个连接读（WAL 模式）。
//! 切换目录时递增代际，旧扫描批次在下一个检查点自行退出，结果不会覆盖新目录。

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::UNIX_EPOCH;

use iv_domain::{GalleryError, GalleryProgress, SortOrder};
use rusqlite::{Connection, params};
use uuid::Uuid;

use crate::schema;
use crate::tags::TagStore;

const CHECK_EVERY: usize = 200;

enum Command {
    Open { dir: PathBuf, token: u64 },
    Refresh { token: u64 },
    Shutdown,
}

/// 后台索引器句柄。Drop 时发关闭命令并等待线程结束。
pub struct Indexer {
    tx: Sender<Command>,
    progress: Arc<Mutex<GalleryProgress>>,
    generation: Arc<AtomicU64>,
    handle: Option<JoinHandle<()>>,
}

impl Indexer {
    pub fn spawn(db_path: PathBuf, sidecar_root: PathBuf) -> Result<Self, GalleryError> {
        // 先在调用线程建库，保证配置错误能返回给调用方。
        let _ = schema::open_or_create(&db_path)?;

        let (tx, rx) = mpsc::channel();
        let progress = Arc::new(Mutex::new(GalleryProgress::default()));
        let generation = Arc::new(AtomicU64::new(0));
        let worker_progress = Arc::clone(&progress);
        let worker_generation = Arc::clone(&generation);
        let handle = thread::Builder::new()
            .name("iv-library-indexer".into())
            .spawn(move || {
                worker(
                    rx,
                    db_path,
                    sidecar_root,
                    worker_progress,
                    worker_generation,
                )
            })
            .map_err(|e| GalleryError::Io(format!("启动索引线程失败：{e}")))?;

        Ok(Self {
            tx,
            progress,
            generation,
            handle: Some(handle),
        })
    }

    /// 切换目录并启动增量索引；立即返回，不等待扫描完成。
    pub fn open(&self, dir: &Path) -> Result<(), GalleryError> {
        let token = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.mark_pending();
        self.tx
            .send(Command::Open {
                dir: dir.to_path_buf(),
                token,
            })
            .map_err(|e| GalleryError::Unavailable(format!("索引线程已退出：{e}")))
    }

    pub fn refresh(&self) -> Result<(), GalleryError> {
        let token = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.mark_pending();
        self.tx
            .send(Command::Refresh { token })
            .map_err(|e| GalleryError::Unavailable(format!("索引线程已退出：{e}")))
    }

    /// 让调用方立即看到“索引中”，避免读到上一轮的 ready 状态。
    fn mark_pending(&self) {
        let mut guard = self
            .progress
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.ready = false;
        guard.degraded = false;
    }

    pub fn progress(&self) -> GalleryProgress {
        let guard = self
            .progress
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        guard.clone()
    }
}

impl Drop for Indexer {
    fn drop(&mut self) {
        let _ = self.tx.send(Command::Shutdown);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn worker(
    rx: Receiver<Command>,
    db_path: PathBuf,
    sidecar_root: PathBuf,
    progress: Arc<Mutex<GalleryProgress>>,
    generation: Arc<AtomicU64>,
) {
    let Ok(mut conn) = schema::open_or_create(&db_path) else {
        mark_degraded(&progress, 0, 0);
        return;
    };
    let tag_store = TagStore::new(sidecar_root);
    let mut current_dir: Option<PathBuf> = None;

    for command in rx {
        let token = match command {
            Command::Shutdown => break,
            Command::Open { dir, token } => {
                if token != generation.load(Ordering::SeqCst) {
                    continue;
                }
                current_dir = Some(dir);
                token
            }
            Command::Refresh { token } => {
                if token != generation.load(Ordering::SeqCst) || current_dir.is_none() {
                    continue;
                }
                token
            }
        };
        let Some(dir) = current_dir.clone() else {
            continue;
        };
        scan_dir(&mut conn, &tag_store, &dir, &progress, &generation, token);
    }
}

fn mark_degraded(progress: &Mutex<GalleryProgress>, indexed: usize, total: usize) {
    let mut guard = progress.lock().unwrap_or_else(|p| p.into_inner());
    guard.indexed = indexed;
    guard.total = total;
    guard.ready = false;
    guard.degraded = true;
}

fn scan_dir(
    conn: &mut Connection,
    tag_store: &TagStore,
    dir: &Path,
    progress: &Mutex<GalleryProgress>,
    generation: &AtomicU64,
    token: u64,
) {
    let paths = iv_files::scan(dir, SortOrder::Name);
    {
        let mut guard = progress.lock().unwrap_or_else(|p| p.into_inner());
        guard.total = paths.len();
        guard.indexed = 0;
        guard.ready = false;
        guard.degraded = false;
    }

    let tx = match conn.transaction() {
        Ok(tx) => tx,
        Err(e) => {
            log::warn!(target: "image_view::library", "开启索引事务失败：{e}");
            mark_degraded(progress, 0, paths.len());
            return;
        }
    };

    let dir_text = dir.to_string_lossy().into_owned();
    let mut fresh: HashSet<String> = HashSet::with_capacity(paths.len());
    let mut latest_revision = 0u64;

    for (index, path) in paths.iter().enumerate() {
        // 切目录后旧批次作废：事务在 return 时回滚，不会污染新目录的索引。
        if index % CHECK_EVERY == 0 && token != generation.load(Ordering::SeqCst) {
            log::debug!(target: "image_view::library", "索引批次已过期，中止扫描");
            return;
        }

        let path_text = path.to_string_lossy().into_owned();
        fresh.insert(path_text.clone());

        let metadata = std::fs::metadata(path).ok();
        let modified = metadata.as_ref().and_then(|m| m.modified().ok());
        let size_bytes = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
        let revision = modified
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let modified_at = modified
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64);
        latest_revision = latest_revision.max(revision);

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let format = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let [width, height] = iv_image_decode::probe_dimensions(path).unwrap_or([0, 0]);

        let id: String = tx
            .query_row(
                "SELECT id FROM images WHERE path = ?1",
                [&path_text],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| Uuid::new_v4().simple().to_string());

        if let Err(e) = tx.execute(
            "INSERT INTO images
                (id, path, dir, name, revision, width, height, format, modified_at, size_bytes, indexed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(path) DO UPDATE SET
                dir = excluded.dir, name = excluded.name, revision = excluded.revision,
                width = excluded.width, height = excluded.height, format = excluded.format,
                modified_at = excluded.modified_at, size_bytes = excluded.size_bytes,
                indexed_at = excluded.indexed_at",
            params![
                id, path_text, dir_text, name, revision, width, height, format,
                modified_at, size_bytes, revision as i64
            ],
        ) {
            log::warn!(target: "image_view::library", "写入索引条目 {} 失败：{e}", path.display());
            continue;
        }

        // 标签权威源是 sidecar：先清空投影，再按 sidecar 回填。
        let _ = tx.execute("DELETE FROM tags WHERE image_id = ?1", [&id]);
        for tag in tag_store.read(path) {
            let _ = tx.execute(
                "INSERT OR IGNORE INTO tags (image_id, tag) VALUES (?1, ?2)",
                params![id, tag],
            );
        }

        if index % 100 == 0 || index + 1 == paths.len() {
            let mut guard = progress.lock().unwrap_or_else(|p| p.into_inner());
            guard.indexed = index + 1;
        }
    }

    // 清理已删除/移走的图片，避免图库出现幽灵条目。
    if let Ok(mut existing) = tx.prepare("SELECT id, path FROM images WHERE dir = ?1") {
        let rows: Vec<(String, String)> = existing
            .query_map([&dir_text], |row| Ok((row.get(0)?, row.get(1)?)))
            .ok()
            .and_then(|iter| iter.filter_map(Result::ok).collect::<Vec<_>>().into())
            .unwrap_or_default();
        drop(existing);
        for (id, path_text) in rows {
            if !fresh.contains(&path_text) {
                let _ = tx.execute("DELETE FROM images WHERE id = ?1", [id]);
            }
        }
    }

    let fingerprint = format!("{}:{}", paths.len(), latest_revision);
    let _ = tx.execute(
        "INSERT INTO directories (dir, fingerprint, indexed_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(dir) DO UPDATE SET fingerprint = excluded.fingerprint,
                                        indexed_at = excluded.indexed_at",
        params![dir_text, fingerprint, latest_revision as i64],
    );

    if let Err(e) = tx.commit() {
        log::warn!(target: "image_view::library", "提交索引事务失败：{e}");
        mark_degraded(progress, 0, paths.len());
        return;
    }

    let mut guard = progress.lock().unwrap_or_else(|p| p.into_inner());
    guard.indexed = paths.len();
    guard.ready = true;
    guard.degraded = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("image-view-indexer-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn progress_starts_empty() {
        let root = temp_root("initial");
        let indexer =
            Indexer::spawn(root.join("library.sqlite3"), root.join("sidecars")).expect("spawn");
        let p = indexer.progress();
        assert_eq!(p.indexed, 0);
        assert_eq!(p.total, 0);
        assert!(!p.ready);
        assert!(!p.degraded);
        drop(indexer);
        std::fs::remove_dir_all(&root).expect("cleanup");
    }
}
