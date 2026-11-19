//! 图片库索引（SQLite）。
//!
//! 当前版本只做 M6 的只读图库：目录索引、筛选、搜索、标签 sidecar 投影。
//! 索引是可重建的加速层；标签权威源在 sidecar，不写原图、不写图片目录。
//!
//! 依赖方向：`iv-application` 定义端口，本 crate 实现；不依赖 GUI，也不把像素写入数据库。

mod indexer;
mod schema;
mod tags;

use std::path::{Path, PathBuf};

use iv_application::GalleryIndex;
use iv_domain::{
    GalleryEntry, GalleryError, GalleryFilter, GalleryPage, GalleryProgress, GallerySortKey,
};
use rusqlite::{Connection, params, params_from_iter, types::Value};
use uuid::Uuid;

use indexer::Indexer;
use tags::TagStore;

/// 基于 SQLite 的图库索引。
pub struct SqliteGalleryIndex {
    /// 查询与标签写入用的连接（WAL 下可与索引线程并发）。
    conn: Connection,
    tag_store: TagStore,
    indexer: Indexer,
    current_dir: Option<PathBuf>,
}

impl SqliteGalleryIndex {
    /// 打开（或新建）索引库并启动后台索引线程。
    pub fn open(db_path: &Path, sidecar_root: &Path) -> Result<Self, GalleryError> {
        let conn = schema::open_or_create(db_path)?;
        let tag_store = TagStore::new(sidecar_root.to_path_buf());
        let indexer = Indexer::spawn(db_path.to_path_buf(), sidecar_root.to_path_buf())?;
        Ok(Self {
            conn,
            tag_store,
            indexer,
            current_dir: None,
        })
    }

    fn sync_tag_projection(&self, path: &Path, tags: &[String]) -> Result<(), GalleryError> {
        let path_text = path.to_string_lossy();
        let id: String = self
            .conn
            .query_row(
                "SELECT id FROM images WHERE path = ?1",
                [&path_text],
                |row| row.get(0),
            )
            .map_err(|e| GalleryError::Io(format!("查找索引条目失败：{e}")))?;

        self.conn
            .execute("DELETE FROM tags WHERE image_id = ?1", [&id])
            .map_err(|e| GalleryError::Io(format!("清空标签投影失败：{e}")))?;
        for tag in tags {
            self.conn
                .execute(
                    "INSERT OR IGNORE INTO tags (image_id, tag) VALUES (?1, ?2)",
                    params![id, tag],
                )
                .map_err(|e| GalleryError::Io(format!("写入标签投影失败：{e}")))?;
        }
        Ok(())
    }
}

impl GalleryIndex for SqliteGalleryIndex {
    fn open(&mut self, dir: &Path) -> Result<GalleryProgress, GalleryError> {
        self.current_dir = Some(dir.to_path_buf());
        self.indexer.open(dir)?;
        Ok(self.indexer.progress())
    }

    fn refresh(&mut self) -> Result<GalleryProgress, GalleryError> {
        self.indexer.refresh()?;
        Ok(self.indexer.progress())
    }

    fn page(
        &self,
        filter: &GalleryFilter,
        offset: usize,
        limit: usize,
    ) -> Result<GalleryPage, GalleryError> {
        let Some(dir) = &self.current_dir else {
            return Ok(GalleryPage {
                entries: Vec::new(),
                total: 0,
            });
        };

        let mut where_clause = String::from("i.dir = ?");
        let mut values: Vec<Value> = vec![Value::Text(dir.to_string_lossy().into_owned())];

        if !filter.query.is_empty() {
            let pattern = format!("%{}%", filter.query);
            where_clause.push_str(
                " AND (i.name LIKE ? OR EXISTS \
                 (SELECT 1 FROM tags t2 WHERE t2.image_id = i.id AND t2.tag LIKE ?))",
            );
            values.push(Value::Text(pattern.clone()));
            values.push(Value::Text(pattern));
        }
        for tag in &filter.tags {
            where_clause.push_str(
                " AND EXISTS (SELECT 1 FROM tags t3 WHERE t3.image_id = i.id AND t3.tag = ?)",
            );
            values.push(Value::Text(tag.clone()));
        }
        if !filter.format.is_empty() {
            where_clause.push_str(" AND i.format = ?");
            values.push(Value::Text(filter.format.to_lowercase()));
        }
        if filter.min_width > 0 {
            where_clause.push_str(" AND i.width >= ?");
            values.push(Value::Integer(filter.min_width as i64));
        }
        if filter.max_width != usize::MAX {
            where_clause.push_str(" AND i.width <= ?");
            values.push(Value::Integer(filter.max_width as i64));
        }
        if let Some(from) = filter.from_date {
            where_clause.push_str(" AND i.modified_at >= ?");
            values.push(Value::Integer(from));
        }
        if let Some(to) = filter.to_date {
            where_clause.push_str(" AND i.modified_at <= ?");
            values.push(Value::Integer(to));
        }

        let total: usize = self
            .conn
            .query_row(
                &format!("SELECT COUNT(*) FROM images i WHERE {where_clause}"),
                params_from_iter(values.iter()),
                |row| row.get(0),
            )
            .map_err(|e| GalleryError::Io(format!("统计图库条目失败：{e}")))?;

        let order_by = match filter.sort {
            GallerySortKey::Name => "i.name COLLATE NOCASE ASC",
            GallerySortKey::Modified => "i.modified_at DESC, i.name COLLATE NOCASE ASC",
            GallerySortKey::Size => "i.size_bytes DESC, i.name COLLATE NOCASE ASC",
            GallerySortKey::Format => "i.format ASC, i.name COLLATE NOCASE ASC",
        };
        let sql = format!(
            "SELECT i.id, i.path, i.name, i.revision, i.width, i.height, i.format,
                    i.modified_at, i.size_bytes,
                    COALESCE((SELECT GROUP_CONCAT(t.tag, char(31)) FROM tags t WHERE t.image_id = i.id), '')
             FROM images i WHERE {where_clause}
             ORDER BY {order_by}, i.id ASC
             LIMIT ? OFFSET ?"
        );
        let mut paged_values = values;
        paged_values.push(Value::Integer(limit as i64));
        paged_values.push(Value::Integer(offset as i64));

        let mut statement = self
            .conn
            .prepare(&sql)
            .map_err(|e| GalleryError::Io(format!("准备图库查询失败：{e}")))?;
        let rows = statement
            .query_map(params_from_iter(paged_values.iter()), |row| {
                let tag_text: String = row.get(9)?;
                let tags = if tag_text.is_empty() {
                    Vec::new()
                } else {
                    tag_text.split('\u{1f}').map(str::to_owned).collect()
                };
                Ok(GalleryEntry {
                    id: row.get(0)?,
                    path: row.get::<_, String>(1)?.into(),
                    name: row.get(2)?,
                    revision: row.get::<_, i64>(3)? as u64,
                    width: row.get::<_, i64>(4)? as usize,
                    height: row.get::<_, i64>(5)? as usize,
                    format: row.get(6)?,
                    modified_at: row.get(7)?,
                    size_bytes: row.get::<_, i64>(8)? as u64,
                    tags,
                })
            })
            .map_err(|e| GalleryError::Io(format!("查询图库条目失败：{e}")))?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(row.map_err(|e| GalleryError::Io(format!("读取图库条目失败：{e}")))?);
        }
        Ok(GalleryPage { entries, total })
    }

    fn add_tag(&mut self, path: &Path, tag: &str) -> Result<(), GalleryError> {
        let tag = tag.trim();
        if tag.is_empty() {
            return Err(GalleryError::InvalidArgument("标签不能为空".into()));
        }
        let mut tags = self.tag_store.read(path);
        if !tags.iter().any(|existing| existing == tag) {
            tags.push(tag.to_owned());
            self.tag_store.write(path, &tags)?;
        }
        self.sync_tag_projection(path, &tags)
    }

    fn remove_tag(&mut self, path: &Path, tag: &str) -> Result<(), GalleryError> {
        let tags: Vec<String> = self
            .tag_store
            .read(path)
            .into_iter()
            .filter(|existing| existing != tag)
            .collect();
        self.tag_store.write(path, &tags)?;
        self.sync_tag_projection(path, &tags)
    }

    fn progress(&self) -> GalleryProgress {
        self.indexer.progress()
    }
}

/// 生成 UUIDv4 句柄（供壳层注册授权表时使用，与图片条目 ID 同一套）。
pub fn new_image_id() -> String {
    Uuid::new_v4().simple().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("image-view-library-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn wait_ready(index: &SqliteGalleryIndex) -> GalleryProgress {
        use std::time::{Duration, Instant};

        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let progress = index.progress();
            if progress.ready || progress.degraded || Instant::now() >= deadline {
                return progress;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn scan_page_filter_and_tag_search() {
        let root = temp_root("scan");
        let photos = root.join("photos");
        std::fs::create_dir_all(&photos).expect("photos");
        for name in ["a.png", "b.png", "photo-c.PNG"] {
            std::fs::write(photos.join(name), b"not a real image").expect("fixture");
        }

        let mut index =
            SqliteGalleryIndex::open(&root.join("library.sqlite3"), &root.join("sidecars"))
                .expect("open");
        index.open(&photos).expect("open dir");
        let progress = wait_ready(&index);
        assert!(progress.ready, "索引应完成：{progress:?}");
        assert_eq!(progress.total, 3);

        let page = index.page(&GalleryFilter::default(), 0, 10).expect("page");
        assert_eq!(page.total, 3);
        assert_eq!(page.entries.len(), 3);
        assert!(page.entries.iter().all(|e| e.id.len() == 32));
        assert!(page.entries.iter().all(|e| e.width == 0)); // 假图片，探测失败

        // 分页
        let second = index
            .page(&GalleryFilter::default(), 2, 10)
            .expect("page 2");
        assert_eq!(second.entries.len(), 1);

        // 文件名子串搜索
        let filter = GalleryFilter {
            query: "photo".into(),
            ..Default::default()
        };
        assert_eq!(index.page(&filter, 0, 10).expect("search").total, 1);

        // 标签写入 sidecar + 投影 + 搜索
        let target = photos.join("a.png");
        index.add_tag(&target, "旅行").expect("add tag");
        assert!(index.tag_store.read(&target).contains(&"旅行".to_owned()));

        let filter = GalleryFilter {
            query: "旅行".into(),
            ..Default::default()
        };
        let tagged = index.page(&filter, 0, 10).expect("tag search");
        assert_eq!(tagged.total, 1);
        assert_eq!(tagged.entries[0].path, target);
        assert_eq!(tagged.entries[0].tags, vec!["旅行".to_owned()]);

        index.remove_tag(&target, "旅行").expect("remove tag");
        assert!(index.tag_store.read(&target).is_empty());
        assert_eq!(index.page(&filter, 0, 10).expect("tag search").total, 0);

        drop(index);
        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn deleted_files_are_removed_on_refresh() {
        let root = temp_root("delete");
        let photos = root.join("photos");
        std::fs::create_dir_all(&photos).expect("photos");
        let gone = photos.join("gone.png");
        std::fs::write(&gone, b"").expect("fixture");
        std::fs::write(photos.join("keep.png"), b"").expect("fixture");

        let mut index =
            SqliteGalleryIndex::open(&root.join("library.sqlite3"), &root.join("sidecars"))
                .expect("open");
        index.open(&photos).expect("open dir");
        assert!(wait_ready(&index).ready);
        assert_eq!(
            index
                .page(&GalleryFilter::default(), 0, 10)
                .expect("page")
                .total,
            2
        );

        std::fs::remove_file(&gone).expect("delete fixture");
        index.refresh().expect("refresh");
        assert!(wait_ready(&index).ready);
        assert_eq!(
            index
                .page(&GalleryFilter::default(), 0, 10)
                .expect("page")
                .total,
            1
        );

        drop(index);
        std::fs::remove_dir_all(&root).expect("cleanup");
    }
}
