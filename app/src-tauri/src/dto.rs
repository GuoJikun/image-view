//! Command/Event 的 DTO 与稳定错误码。
//!
//! 前端只依赖这些轻量结构；像素数据不进入 DTO，而是通过 `iviewer://` 协议按
//! ImageId + revision 读取。

use serde::{Deserialize, Serialize};

use iv_image_decode::LoadError;

/// 命令错误：`code` 供前端分支，`message` 保留底层细节用于日志/提示。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandError {
    pub code: &'static str,
    pub message: String,
}

impl CommandError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn io(message: impl Into<String>) -> Self {
        Self::new("io", message)
    }

    pub fn invalid_path(message: impl Into<String>) -> Self {
        Self::new("invalid_path", message)
    }
}

impl From<LoadError> for CommandError {
    fn from(error: LoadError) -> Self {
        let code = match &error {
            LoadError::Io(_) => "io",
            LoadError::Decode(_) => "decode",
            LoadError::TooLarge { .. } => "too_large",
            LoadError::OutOfMemory => "out_of_memory",
            LoadError::PreloadSkipped { .. } => "preload_skipped",
            #[cfg(feature = "avif")]
            LoadError::Avif(_) => "decode",
            #[cfg(feature = "heic")]
            LoadError::Heic(_) => "decode",
            #[cfg(feature = "svg")]
            LoadError::Svg(_) => "decode",
            LoadError::MissingDecoder { .. } => "missing_decoder",
            #[cfg(feature = "svg")]
            LoadError::SvgzUnsupported => "unsupported",
        };
        Self::new(code, error.to_string())
    }
}

/// 目录内的一张图片。`id` 是后端生成的授权句柄，前端不能凭路径直接读资源。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryEntryDto {
    pub id: String,
    pub path: String,
    pub name: String,
    /// 文件修改时间（毫秒），进入缩略图 URL 以刷新 WebView 缓存。
    pub revision: u64,
}

/// 一次目录扫描的完整快照。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorySnapshot {
    pub dir: String,
    pub entries: Vec<DirectoryEntryDto>,
    pub index: Option<usize>,
    pub position: Option<(usize, usize)>,
    pub can_prev: bool,
    pub can_next: bool,
    pub current: Option<DirectoryEntryDto>,
    /// `name` 或 `modified`。
    pub sort: String,
}

/// `request_preview` 的返回值；事件会带回同一个 requestId。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewRequestDto {
    pub request_id: u64,
    pub image_id: String,
    pub revision: u64,
}

/// 预览解码完成。URL 由前端按 `imageId + revision` 拼接，payload 不含像素。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewReadyDto {
    pub request_id: u64,
    pub image_id: String,
    pub revision: u64,
    pub path: String,
    pub file_name: String,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub source_width: u32,
    pub source_height: u32,
    pub downscaled: bool,
    pub source_depth: u8,
    pub color_converted: bool,
    pub color_source: Option<String>,
    pub file_size: u64,
    pub modified_at: Option<i64>,
}

/// 预览解码失败；过期请求的失败事件会被前端丢弃。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewFailedDto {
    pub request_id: u64,
    pub image_id: String,
    pub code: &'static str,
    pub message: String,
}

/// 动画降级提示：动画本应能播却被降级为静态首帧（超预算 / 16-bit / 解码失败）。
/// `code` 供前端本地化，`detail` 保留底层数值/错误用于日志，不直接展示。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DegradedDto {
    pub request_id: u64,
    pub image_id: String,
    pub code: &'static str,
    pub detail: String,
}

/// 动画帧解码完成。首帧静态预览先由 `preview-ready` 送出，完整帧后到。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnimationReadyDto {
    pub request_id: u64,
    pub image_id: String,
    pub revision: u64,
    pub frame_count: u32,
    /// 总播放轮数；`None` 表示无限循环。
    pub loops: Option<u32>,
    /// 每帧间隔（毫秒），已在解码层钳制到 50~1000。
    pub delays: Vec<u32>,
}

/// 界面偏好：启动时读取，变化时整体写回（对应 `settings.ron` 里的偏好字段）。
///
/// 唯一持久化位置是 `settings.ron`；前端 localStorage 只作首帧缓存与旧版本迁移来源。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiPreferencesDto {
    pub show_thumbnails: bool,
    pub show_color_status: bool,
    /// `system` / `light` / `dark`。
    pub theme: String,
    /// `system` / `zh` / `en`。
    pub language: String,
    pub check_updates_on_startup: bool,
}

/// 菜单可勾选项的当前状态（`sync_native_menu` 入参）。
///
/// 自绘菜单（Windows / Linux）直接用前端同一份状态渲染，这里只负责 macOS 原生菜单：
/// 语言不变时按 [`MenuStateDto::checks`] 增量 `set_checked`，不必重建菜单。
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MenuStateDto {
    pub thumbnails: bool,
    pub slideshow: bool,
    pub color_badge: bool,
    pub metadata: bool,
    pub auto_update: bool,
    /// `name` 或 `modified`。
    pub sort: String,
    /// `system` / `light` / `dark`。
    pub theme: String,
    /// `system` / `zh` / `en`。
    pub language: String,
}

impl MenuStateDto {
    /// 可勾选菜单项的动作 id 与勾选值，顺序同菜单自上而下。
    ///
    /// id 必须与原生菜单里的菜单项 id、前端 `actions.ts` 的动作 id 三处一致；
    /// 原生菜单缺少某个 id 时会记一条警告（见 `menu::NativeMenu::apply`）。
    pub fn checks(&self) -> [(&'static str, bool); 13] {
        [
            ("toggle-thumbnails", self.thumbnails),
            ("slideshow", self.slideshow),
            ("sort-name", self.sort == "name"),
            ("sort-modified", self.sort == "modified"),
            ("theme-system", self.theme == "system"),
            ("theme-light", self.theme == "light"),
            ("theme-dark", self.theme == "dark"),
            ("lang-system", self.language == "system"),
            ("lang-zh", self.language == "zh"),
            ("lang-en", self.language == "en"),
            ("color-badge", self.color_badge),
            ("metadata", self.metadata),
            ("auto-update", self.auto_update),
        ]
    }
}

/// 文件扩展名 → 展示用格式名。
pub fn format_label(path: &std::path::Path) -> String {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_uppercase())
        .unwrap_or_else(|| "UNKNOWN".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn format_label_is_uppercase_and_handles_missing_extension() {
        assert_eq!(format_label(Path::new("a.JpEg")), "JPEG");
        assert_eq!(format_label(Path::new("photo.heic")), "HEIC");
        assert_eq!(format_label(Path::new("no-extension")), "UNKNOWN");
    }

    #[test]
    fn command_error_keeps_stable_code() {
        let error = CommandError::from(LoadError::TooLarge { w: 1, h: 2 });
        assert_eq!(error.code, "too_large");
    }

    fn menu_state() -> MenuStateDto {
        MenuStateDto {
            thumbnails: true,
            slideshow: false,
            color_badge: true,
            metadata: false,
            auto_update: true,
            sort: "modified".to_owned(),
            theme: "light".to_owned(),
            language: "en".to_owned(),
        }
    }

    fn checked(checks: &[(&'static str, bool)], id: &str) -> Option<bool> {
        checks
            .iter()
            .find(|(name, _)| *name == id)
            .map(|(_, value)| *value)
    }

    #[test]
    fn menu_checks_follow_state() {
        let checks = menu_state().checks();
        assert_eq!(checked(&checks, "toggle-thumbnails"), Some(true));
        assert_eq!(checked(&checks, "slideshow"), Some(false));
        assert_eq!(checked(&checks, "color-badge"), Some(true));
        assert_eq!(checked(&checks, "metadata"), Some(false));
        assert_eq!(checked(&checks, "auto-update"), Some(true));
    }

    /// 排序 / 主题 / 语言是三选一：任何时刻恰好一项勾选。
    #[test]
    fn menu_checks_pick_exactly_one_per_group() {
        let checks = menu_state().checks();
        let groups: [&[&str]; 3] = [
            &["sort-name", "sort-modified"],
            &["theme-system", "theme-light", "theme-dark"],
            &["lang-system", "lang-zh", "lang-en"],
        ];
        for group in groups {
            let hits = group
                .iter()
                .filter(|id| checked(&checks, id) == Some(true))
                .count();
            assert_eq!(hits, 1, "{group:?} 应恰好勾选一项");
        }
        assert_eq!(checked(&checks, "sort-name"), Some(false));
        assert_eq!(checked(&checks, "theme-light"), Some(true));
        assert_eq!(checked(&checks, "lang-en"), Some(true));
    }

    /// 未知取值不能让任何一项被误勾选（前端传错时菜单保持干净，而不是显示错误状态）。
    #[test]
    fn unknown_enum_values_check_nothing() {
        let checks = MenuStateDto {
            sort: "size".to_owned(),
            theme: "auto".to_owned(),
            language: "fr".to_owned(),
            ..menu_state()
        }
        .checks();
        assert!(checks.iter().all(|(id, checked)| {
            !(id.starts_with("sort-") || id.starts_with("theme-") || id.starts_with("lang-"))
                || !checked
        }));
    }

    /// 菜单项 id 不能有重复，否则后台只会更新其中一个。
    #[test]
    fn menu_check_ids_are_unique() {
        let checks = menu_state().checks();
        for (index, (id, _)) in checks.iter().enumerate() {
            assert!(
                !checks[..index].iter().any(|(other, _)| other == id),
                "重复的菜单项 id：{id}"
            );
        }
    }

    /// 前端按 camelCase 发送（`colorBadge` / `autoUpdate`），字段名对不上会整条命令失败。
    #[test]
    fn menu_state_deserializes_from_frontend_payload() {
        let json = r#"{
            "thumbnails": true,
            "slideshow": false,
            "colorBadge": true,
            "metadata": false,
            "autoUpdate": true,
            "sort": "name",
            "theme": "dark",
            "language": "zh"
        }"#;
        let state: MenuStateDto = serde_json::from_str(json).expect("deserialize");
        assert!(state.color_badge);
        assert!(state.auto_update);
        assert_eq!(state.theme, "dark");
        let checks = state.checks();
        assert_eq!(checked(&checks, "theme-dark"), Some(true));
        assert_eq!(checked(&checks, "lang-zh"), Some(true));
    }
}
