//! 标签 sidecar：应用数据目录下的 JSON，是标签的权威源。
//!
//! 原图所在目录保持只读；sidecar 不污染原图目录，也不写进原图字节。
//! 写失败（只读/ACL）只记录日志并返回错误，调用方据此保持索引可用。

use std::path::{Path, PathBuf};

use iv_domain::GalleryError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const SIDECAR_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Sidecar {
    version: u32,
    path: String,
    tags: Vec<String>,
}

/// 标签 sidecar 仓库。
#[derive(Clone, Debug)]
pub struct TagStore {
    root: PathBuf,
}

impl TagStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// sidecar 文件名按原图绝对路径做 UUIDv5，稳定且不含用户路径片段。
    pub fn sidecar_path(&self, image: &Path) -> PathBuf {
        let key = image.to_string_lossy();
        let id = Uuid::new_v5(&Uuid::NAMESPACE_OID, key.as_bytes()).simple();
        self.root.join(format!("{id}.json"))
    }

    /// 读取标签；文件不存在或内容损坏时返回空列表（不 panic、不阻塞）。
    pub fn read(&self, image: &Path) -> Vec<String> {
        let path = self.sidecar_path(image);
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Vec::new();
        };
        match serde_json::from_str::<Sidecar>(&text) {
            Ok(sidecar) if sidecar.version == SIDECAR_VERSION => sidecar
                .tags
                .into_iter()
                .map(|tag| tag.trim().to_owned())
                .filter(|tag| !tag.is_empty())
                .collect(),
            Ok(_) => {
                log::warn!(target: "image_view::library", "sidecar 版本不兼容：{}", path.display());
                Vec::new()
            }
            Err(e) => {
                log::warn!(target: "image_view::library", "sidecar 损坏：{}（{e}）", path.display());
                Vec::new()
            }
        }
    }

    /// 原子写入标签（临时文件 + rename），返回 IO 错误供调用方降级。
    pub fn write(&self, image: &Path, tags: &[String]) -> Result<(), GalleryError> {
        std::fs::create_dir_all(&self.root).map_err(|e| {
            GalleryError::Io(format!(
                "创建 sidecar 目录 {} 失败：{e}",
                self.root.display()
            ))
        })?;

        let sidecar = Sidecar {
            version: SIDECAR_VERSION,
            path: image.to_string_lossy().into_owned(),
            tags: tags.to_vec(),
        };
        let text = serde_json::to_string_pretty(&sidecar)
            .map_err(|e| GalleryError::Io(format!("序列化 sidecar 失败：{e}")))?;

        let final_path = self.sidecar_path(image);
        let temp_path = final_path.with_extension("json.tmp");
        std::fs::write(&temp_path, text).map_err(|e| {
            GalleryError::Io(format!("写入 sidecar {} 失败：{e}", temp_path.display()))
        })?;
        std::fs::rename(&temp_path, &final_path).map_err(|e| {
            GalleryError::Io(format!("提交 sidecar {} 失败：{e}", final_path.display()))
        })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("image-view-tags-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    #[test]
    fn sidecar_round_trip() {
        let root = temp_root("roundtrip");
        let store = TagStore::new(root.clone());
        let image = Path::new("E:/photos/IMG_0001.CR3");
        let tags = vec!["旅行".to_owned(), "海边".to_owned()];

        store.write(image, &tags).expect("write");
        assert_eq!(store.read(image), tags);

        // sidecar 文件名稳定，且不含原图路径片段。
        assert_eq!(store.sidecar_path(image), store.sidecar_path(image));
        assert!(
            !store
                .sidecar_path(image)
                .to_string_lossy()
                .contains("IMG_0001")
        );

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn corrupt_sidecar_reads_as_empty() {
        let root = temp_root("corrupt");
        let store = TagStore::new(root.clone());
        let image = Path::new("E:/photos/bad.png");
        std::fs::write(store.sidecar_path(image), b"{not json").expect("write corrupt");
        assert!(store.read(image).is_empty());
        std::fs::remove_dir_all(&root).expect("cleanup");
    }
}
