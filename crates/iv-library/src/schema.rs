//! SQLite 索引的表结构与版本管理。
//!
//! 索引是可重建的加速层：`user_version` 不符或表损坏时直接重建，
//! 不尝试猜测旧结构。标签权威源在 sidecar，重建后由索引器回填。

use std::path::Path;

use iv_domain::GalleryError;
use rusqlite::Connection;

/// 当前 schema 版本。每次改表结构都要 +1 并写迁移/重建测试。
pub const SCHEMA_VERSION: u32 = 1;

/// 打开索引库，必要时建表或重建。
pub fn open_or_create(path: &Path) -> Result<Connection, GalleryError> {
    let conn = Connection::open(path)
        .map_err(|e| GalleryError::Io(format!("打开索引库 {} 失败：{e}", path.display())))?;
    configure(&conn)?;

    let version: u32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|e| GalleryError::Io(format!("读取索引版本失败：{e}")))?;
    if version != SCHEMA_VERSION {
        reset(&conn)?;
        conn.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION};"))
            .map_err(|e| GalleryError::Io(format!("写入索引版本失败：{e}")))?;
    }
    ensure_tables(&conn)?;
    Ok(conn)
}

fn configure(conn: &Connection) -> Result<(), GalleryError> {
    // WAL 允许索引线程写、查询端同时读；外键约束保证 tags 不残留孤儿行。
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
        .map_err(|e| GalleryError::Io(format!("配置索引库失败：{e}")))
}

fn reset(conn: &Connection) -> Result<(), GalleryError> {
    log::warn!(target: "image_view::library", "索引版本不匹配，重建索引表");
    conn.execute_batch(
        "DROP TABLE IF EXISTS tags;
         DROP TABLE IF EXISTS images;
         DROP TABLE IF EXISTS directories;",
    )
    .map_err(|e| GalleryError::Io(format!("重建索引表失败：{e}")))
}

fn ensure_tables(conn: &Connection) -> Result<(), GalleryError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS images (
            id           TEXT PRIMARY KEY,
            path         TEXT NOT NULL UNIQUE,
            dir          TEXT NOT NULL,
            name         TEXT NOT NULL,
            revision     INTEGER NOT NULL,
            width        INTEGER NOT NULL DEFAULT 0,
            height       INTEGER NOT NULL DEFAULT 0,
            format       TEXT NOT NULL,
            modified_at  INTEGER,
            size_bytes   INTEGER NOT NULL,
            indexed_at   INTEGER NOT NULL
         );
         CREATE INDEX IF NOT EXISTS idx_images_dir  ON images(dir);
         CREATE INDEX IF NOT EXISTS idx_images_name ON images(name COLLATE NOCASE);
         CREATE INDEX IF NOT EXISTS idx_images_mtime ON images(modified_at);

         CREATE TABLE IF NOT EXISTS tags (
            image_id TEXT NOT NULL REFERENCES images(id) ON DELETE CASCADE,
            tag      TEXT NOT NULL,
            PRIMARY KEY (image_id, tag)
         );
         CREATE INDEX IF NOT EXISTS idx_tags_tag ON tags(tag);

         CREATE TABLE IF NOT EXISTS directories (
            dir         TEXT PRIMARY KEY,
            fingerprint TEXT NOT NULL,
            indexed_at  INTEGER NOT NULL
         );",
    )
    .map_err(|e| GalleryError::Io(format!("创建索引表失败：{e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(tag: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("image-view-schema-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir.join("library.sqlite3")
    }

    #[test]
    fn open_or_create_builds_all_tables_and_version() {
        let path = temp_db("create");
        let conn = open_or_create(&path).expect("open");
        let version: u32 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .expect("version");
        assert_eq!(version, SCHEMA_VERSION);

        for table in ["images", "tags", "directories"] {
            let exists: bool = conn
                .query_row(
                    "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
                    [table],
                    |_| Ok(true),
                )
                .unwrap_or(false);
            assert!(exists, "缺少表 {table}");
        }
        drop(conn);
        std::fs::remove_dir_all(path.parent().expect("parent")).expect("cleanup");
    }

    #[test]
    fn mismatched_version_rebuilds_without_data() {
        let path = temp_db("rebuild");
        {
            let conn = open_or_create(&path).expect("open");
            conn.execute_batch("PRAGMA user_version = 99;")
                .expect("bump");
        }
        {
            let conn = open_or_create(&path).expect("reopen rebuilds");
            let version: u32 = conn
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .expect("version");
            assert_eq!(version, SCHEMA_VERSION);
        }
        std::fs::remove_dir_all(path.parent().expect("parent")).expect("cleanup");
    }
}
