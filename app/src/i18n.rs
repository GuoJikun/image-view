//! 界面多语言：中文 / 英文静态文案表。
//!
//! 用词与 `main` 分支的 `app/src/locales/*.json` 保持一致，但这里改用 Rust 静态表：
//! 两种语言填同一个结构体，漏翻或多写字段都会直接编译失败，运行时不需要解析 JSON。
//!
//! 约定：
//! - GUI 层（`view/` / `widgets/` / 适配器）只从这里取文案，不再写中文字面量；
//! - 语言选择由 [`iv_domain::Language`] 决定，`System` 时按系统标签回退（非中文→英文）；
//! - 带参数的文案走方法而不是 `format!`，由文案表决定标点与语序。

use std::time::{Duration, SystemTime};

use iv_domain::Language;

/// 已解析的界面语言。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Zh,
    En,
}

macro_rules! text_table {
    ($($field:ident),+ $(,)?) => {
        /// 一份完整的界面文案。`lang` 用于需要按语序组句的地方。
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct Text {
            pub lang: Lang,
            $(pub $field: &'static str,)+
        }

        impl Text {
            /// 全部可翻译字段，供测试做完整性检查。
            #[cfg(test)]
            pub fn entries(&self) -> Vec<(&'static str, &'static str)> {
                vec![$((stringify!($field), self.$field),)+]
            }
        }
    };
}

text_table! {
    app_name,

    // 菜单：分组
    menu_file,
    menu_view,
    menu_help,

    // 菜单：文件
    menu_open,
    menu_reveal,
    menu_copy_path,
    menu_trash,

    // 菜单：显示
    menu_thumbnails,
    menu_slideshow,
    menu_sort,
    menu_sort_name,
    menu_sort_modified,
    menu_zoom_in,
    menu_zoom_out,
    menu_fit,
    menu_actual,
    menu_theme,
    menu_theme_system,
    menu_theme_light,
    menu_theme_dark,
    menu_language,
    menu_lang_system,
    menu_lang_zh,
    menu_lang_en,
    menu_color_badge,
    menu_metadata,

    // 菜单：帮助
    menu_shortcuts,
    menu_logs,

    // 空状态
    empty_title,
    empty_hint,
    empty_open,
    empty_drag,

    // 状态栏与横幅
    status_no_image,
    status_downscaled,
    status_slideshow,
    status_thumbnails,
    status_loading,
    banner_dismiss,

    // 元数据面板
    metadata_empty,
    metadata_format,
    metadata_file_name,
    metadata_dimensions,
    metadata_file_size,
    metadata_modified,
    metadata_depth,
    metadata_color,
    metadata_locate,
    metadata_copy_path,
    metadata_unknown,
    color_converted,

    // 提示
    notice_dir_empty,
    notice_path_missing,
    notice_trashed,

    // 错误
    error_read_file,
    error_decode,
    error_too_large,
    error_out_of_memory,
    error_preload_skipped,
    error_avif,
    error_heic,
    error_svg,
    error_missing_decoder,
    error_svgz,
    error_trash,
    error_reveal,
    error_open,

    // 弹窗
    dialog_shortcuts,
    dialog_close,
    dialog_images,

    // 快捷键说明
    shortcut_open,
    shortcut_navigate,
    shortcut_zoom,
    shortcut_fit,
    shortcut_actual,
    shortcut_rotate,
    shortcut_playback,
    shortcut_fullscreen,
    shortcut_trash,
    shortcut_copy_path,
    shortcut_dismiss,
    shortcut_pan,
    shortcut_wheel,
}

pub static ZH: Text = Text {
    lang: Lang::Zh,
    app_name: "图片查看器",

    menu_file: "文件",
    menu_view: "显示",
    menu_help: "帮助",

    menu_open: "打开图片…",
    menu_reveal: "在文件管理器中显示",
    menu_copy_path: "复制路径",
    menu_trash: "移到废纸篓",

    menu_thumbnails: "缩略图条",
    menu_slideshow: "幻灯片",
    menu_sort: "排序",
    menu_sort_name: "按名称",
    menu_sort_modified: "按修改时间",
    menu_zoom_in: "放大",
    menu_zoom_out: "缩小",
    menu_fit: "适配窗口",
    menu_actual: "实际大小",
    menu_theme: "主题",
    menu_theme_system: "跟随系统",
    menu_theme_light: "浅色",
    menu_theme_dark: "深色",
    menu_language: "语言",
    menu_lang_system: "跟随系统",
    menu_lang_zh: "简体中文",
    menu_lang_en: "English",
    menu_color_badge: "显示色彩转换标记",
    menu_metadata: "元数据面板",

    menu_shortcuts: "快捷键…",
    menu_logs: "打开日志文件夹",

    empty_title: "还没有打开图片",
    empty_hint: "从文件打开，或把图片拖进窗口",
    empty_open: "打开图片",
    empty_drag: "松开以打开",

    status_no_image: "未打开图片",
    status_downscaled: "已降采样",
    status_slideshow: "幻灯片中",
    status_thumbnails: "缩略图生成中…",
    status_loading: "正在加载…",
    banner_dismiss: "关闭",

    metadata_empty: "未打开图片",
    metadata_format: "格式",
    metadata_file_name: "文件名",
    metadata_dimensions: "尺寸",
    metadata_file_size: "文件大小",
    metadata_modified: "修改时间",
    metadata_depth: "位深",
    metadata_color: "色彩",
    metadata_locate: "定位",
    metadata_copy_path: "复制路径",
    metadata_unknown: "未知",
    color_converted: "已转换到",

    notice_dir_empty: "目录中没有可显示的图片",
    notice_path_missing: "路径不存在或无法读取",
    notice_trashed: "已移到回收站",

    error_read_file: "读取文件失败",
    error_decode: "图片已损坏或格式不受支持",
    error_too_large: "图片过大，超过安全上限",
    error_out_of_memory: "解码超出内存上限",
    error_preload_skipped: "已跳过大图预加载",
    error_avif: "AVIF 解码失败",
    error_heic: "HEIC 解码失败",
    error_svg: "SVG 解析失败",
    error_missing_decoder: "当前构建未包含该格式的解码器，请使用启用对应特性的版本",
    error_svgz: "不支持 Gzip 压缩的 SVGZ，请先解压为 .svg",
    error_trash: "移入回收站失败",
    error_reveal: "无法在文件管理器中定位",
    error_open: "无法打开",

    dialog_shortcuts: "快捷键",
    dialog_close: "关闭",
    dialog_images: "图片",

    shortcut_open: "打开图片",
    shortcut_navigate: "上一张 / 下一张",
    shortcut_zoom: "放大 / 缩小",
    shortcut_fit: "适配窗口",
    shortcut_actual: "实际大小",
    shortcut_rotate: "顺时针 / 逆时针旋转",
    shortcut_playback: "幻灯片播放 / 暂停",
    shortcut_fullscreen: "全屏",
    shortcut_trash: "移到废纸篓",
    shortcut_copy_path: "复制路径",
    shortcut_dismiss: "关闭菜单、提示或对话框",
    shortcut_pan: "拖动平移",
    shortcut_wheel: "滚轮以光标为中心缩放",
};

pub static EN: Text = Text {
    lang: Lang::En,
    app_name: "Image Viewer",

    menu_file: "File",
    menu_view: "View",
    menu_help: "Help",

    menu_open: "Open Image…",
    menu_reveal: "Reveal in File Manager",
    menu_copy_path: "Copy Path",
    menu_trash: "Move to Trash",

    menu_thumbnails: "Thumbnails",
    menu_slideshow: "Slideshow",
    menu_sort: "Sort",
    menu_sort_name: "By Name",
    menu_sort_modified: "By Date Modified",
    menu_zoom_in: "Zoom In",
    menu_zoom_out: "Zoom Out",
    menu_fit: "Fit to Window",
    menu_actual: "Actual Size",
    menu_theme: "Theme",
    menu_theme_system: "System",
    menu_theme_light: "Light",
    menu_theme_dark: "Dark",
    menu_language: "Language",
    menu_lang_system: "System",
    menu_lang_zh: "简体中文",
    menu_lang_en: "English",
    menu_color_badge: "Show Color Conversion Badge",
    menu_metadata: "Metadata Panel",

    menu_shortcuts: "Keyboard Shortcuts…",
    menu_logs: "Open Logs Folder",

    empty_title: "No image open",
    empty_hint: "Open a file, or drag an image into the window",
    empty_open: "Open Image",
    empty_drag: "Drop to open",

    status_no_image: "No image",
    status_downscaled: "Downsampled",
    status_slideshow: "Slideshow",
    status_thumbnails: "Generating thumbnails…",
    status_loading: "Loading…",
    banner_dismiss: "Dismiss",

    metadata_empty: "No image open",
    metadata_format: "Format",
    metadata_file_name: "File name",
    metadata_dimensions: "Dimensions",
    metadata_file_size: "File size",
    metadata_modified: "Modified",
    metadata_depth: "Bit depth",
    metadata_color: "Color",
    metadata_locate: "Reveal",
    metadata_copy_path: "Copy path",
    metadata_unknown: "Unknown",
    color_converted: "Converted to",

    notice_dir_empty: "No displayable image in this folder",
    notice_path_missing: "Path is missing or unreadable",
    notice_trashed: "Moved to Trash",

    error_read_file: "Failed to read file",
    error_decode: "Image is corrupt or in an unsupported format",
    error_too_large: "Image is too large and exceeds the safety limit",
    error_out_of_memory: "Decoding exceeded the memory limit",
    error_preload_skipped: "Skipped preloading a large image",
    error_avif: "AVIF decoding failed",
    error_heic: "HEIC decoding failed",
    error_svg: "SVG parsing failed",
    error_missing_decoder: "This build has no decoder for the format; use a build with that feature enabled",
    error_svgz: "Gzip-compressed SVGZ is not supported; extract it to .svg first",
    error_trash: "Failed to move to Trash",
    error_reveal: "Cannot reveal in the file manager",
    error_open: "Cannot open",

    dialog_shortcuts: "Keyboard Shortcuts",
    dialog_close: "Close",
    dialog_images: "Images",

    shortcut_open: "Open image",
    shortcut_navigate: "Previous / next image",
    shortcut_zoom: "Zoom in / out",
    shortcut_fit: "Fit to window",
    shortcut_actual: "Actual size",
    shortcut_rotate: "Rotate clockwise / counter-clockwise",
    shortcut_playback: "Start / pause slideshow",
    shortcut_fullscreen: "Fullscreen",
    shortcut_trash: "Move to Trash",
    shortcut_copy_path: "Copy path",
    shortcut_dismiss: "Dismiss menu, banner or dialog",
    shortcut_pan: "Drag to pan",
    shortcut_wheel: "Wheel to zoom at cursor",
};

/// 按已解析的语言标签取文案表；未识别的标签回退中文（与 `iv-domain` 的解析方向一致）。
pub fn for_tag(tag: &str) -> &'static Text {
    if tag.starts_with("en") { &EN } else { &ZH }
}

/// 按语言偏好与系统标签取文案表。
pub fn for_language(language: Language, system: Option<&str>) -> &'static Text {
    for_tag(language.resolve_tag(system))
}

impl Text {
    /// 把「前缀 + 细节」拼成一句话；中英文的冒号形式不同。
    pub fn detail(&self, prefix: &str, detail: &str) -> String {
        match self.lang {
            Lang::Zh => format!("{prefix}：{detail}"),
            Lang::En => format!("{prefix}: {detail}"),
        }
    }

    /// 空状态提示 + 快捷键（括号形式随语言变化）。
    pub fn empty_hint_with(&self, accelerator: &str) -> String {
        match self.lang {
            Lang::Zh => format!("{}（{}）", self.empty_hint, accelerator),
            Lang::En => format!("{} ({})", self.empty_hint, accelerator),
        }
    }

    /// 原图与显示尺寸不一致时的维度文案。
    pub fn dimensions_with_display(&self, source: [u32; 2], decoded: [u32; 2]) -> String {
        match self.lang {
            Lang::Zh => format!(
                "{}×{}（显示 {}×{}）",
                source[0], source[1], decoded[0], decoded[1]
            ),
            Lang::En => format!(
                "{}×{} (shown {}×{})",
                source[0], source[1], decoded[0], decoded[1]
            ),
        }
    }

    /// 图片超过安全上限。
    pub fn image_too_large(&self, width: u32, height: u32) -> String {
        match self.lang {
            Lang::Zh => format!("{}（{width}×{height}）", self.error_too_large),
            Lang::En => format!("{} ({width}×{height})", self.error_too_large),
        }
    }

    /// 大图跳过预加载。
    pub fn preload_skipped(&self, width: u32, height: u32) -> String {
        match self.lang {
            Lang::Zh => format!("{}（{width}×{height}）", self.error_preload_skipped),
            Lang::En => format!("{} ({width}×{height})", self.error_preload_skipped),
        }
    }

    /// 缺少解码器时提示具体格式与扩展名。
    pub fn missing_decoder(&self, format: &str, extension: &str) -> String {
        match self.lang {
            Lang::Zh => format!("{}（{format} / .{extension}）", self.error_missing_decoder),
            Lang::En => format!("{} ({format} / .{extension})", self.error_missing_decoder),
        }
    }

    /// 文件已移入回收站的提示。
    pub fn trashed_notice(&self, name: &str) -> String {
        self.detail(self.notice_trashed, name)
    }

    /// 目录里没有图片的提示。
    pub fn dir_empty_notice(&self, path: &str) -> String {
        self.detail(self.notice_dir_empty, path)
    }

    /// 路径不存在的提示。
    pub fn path_missing_notice(&self, path: &str) -> String {
        self.detail(self.notice_path_missing, path)
    }

    /// 相对修改时间；不引入额外时区依赖。
    pub fn relative_age(&self, modified: Option<SystemTime>) -> String {
        let Some(modified) = modified else {
            return self.metadata_unknown.to_owned();
        };
        let elapsed = SystemTime::now()
            .duration_since(modified)
            .unwrap_or(Duration::ZERO);
        let seconds = elapsed.as_secs();

        match self.lang {
            Lang::Zh => {
                if seconds < 60 {
                    "刚刚".to_owned()
                } else if seconds < 3_600 {
                    format!("{} 分钟前", seconds / 60)
                } else if seconds < 86_400 {
                    format!("{} 小时前", seconds / 3_600)
                } else if seconds < 86_400 * 30 {
                    format!("{} 天前", seconds / 86_400)
                } else if seconds < 86_400 * 365 {
                    format!("{} 个月前", seconds / (86_400 * 30))
                } else {
                    format!("{} 年前", seconds / (86_400 * 365))
                }
            }
            Lang::En => {
                if seconds < 60 {
                    "just now".to_owned()
                } else if seconds < 3_600 {
                    plural(seconds / 60, "minute")
                } else if seconds < 86_400 {
                    plural(seconds / 3_600, "hour")
                } else if seconds < 86_400 * 30 {
                    plural(seconds / 86_400, "day")
                } else if seconds < 86_400 * 365 {
                    plural(seconds / (86_400 * 30), "month")
                } else {
                    plural(seconds / (86_400 * 365), "year")
                }
            }
        }
    }
}

fn plural(count: u64, unit: &str) -> String {
    if count == 1 {
        format!("1 {unit} ago")
    } else {
        format!("{count} {unit}s ago")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 两份文案表必须字段齐全且非空；漏填会让界面出现空白。
    #[test]
    fn both_tables_are_complete() {
        for text in [&ZH, &EN] {
            let entries = text.entries();
            assert!(entries.len() > 50, "文案字段数量异常：{}", entries.len());
            for (name, value) in entries {
                assert!(!value.is_empty(), "{:?} 的 {name} 为空", text.lang);
            }
        }
    }

    /// 键顺序由宏生成，两份表必须一一对应，避免某语言多写/漏写。
    #[test]
    fn tables_share_the_same_keys() {
        let zh: Vec<_> = ZH.entries().into_iter().map(|(name, _)| name).collect();
        let en: Vec<_> = EN.entries().into_iter().map(|(name, _)| name).collect();
        assert_eq!(zh, en);
    }

    /// 语言标签决定取哪张表；`en-US` 取英文，其余（含未识别标签）回退中文。
    #[test]
    fn tags_select_the_expected_table() {
        assert_eq!(for_tag("zh"), &ZH);
        assert_eq!(for_tag("en"), &EN);
        assert_eq!(for_tag("en-US"), &EN);
        assert_eq!(for_tag("fr"), &ZH);
    }

    #[test]
    fn language_preference_overrides_system_tag() {
        assert_eq!(for_language(Language::En, Some("zh-CN")), &EN);
        assert_eq!(for_language(Language::Zh, Some("en-US")), &ZH);
        assert_eq!(for_language(Language::System, Some("zh-CN")), &ZH);
        assert_eq!(for_language(Language::System, Some("ja-JP")), &EN);
        assert_eq!(for_language(Language::System, None), &EN);
    }

    /// 带参数的句子要用各自语言的标点，不能把中文冒号带进英文界面。
    #[test]
    fn composed_sentences_follow_each_language() {
        assert_eq!(
            ZH.detail("读取文件失败", "权限不足"),
            "读取文件失败：权限不足"
        );
        assert_eq!(
            EN.detail("Failed to read file", "denied"),
            "Failed to read file: denied"
        );
        assert_eq!(
            ZH.dimensions_with_display([4000, 3000], [2000, 1500]),
            "4000×3000（显示 2000×1500）"
        );
        assert_eq!(
            EN.dimensions_with_display([4000, 3000], [2000, 1500]),
            "4000×3000 (shown 2000×1500)"
        );
        assert_eq!(ZH.trashed_notice("a.png"), "已移到回收站：a.png");
        assert_eq!(EN.trashed_notice("a.png"), "Moved to Trash: a.png");
    }

    #[test]
    fn relative_age_is_localized() {
        let now = SystemTime::now();
        assert_eq!(ZH.relative_age(Some(now - Duration::from_secs(5))), "刚刚");
        assert_eq!(
            ZH.relative_age(Some(now - Duration::from_secs(120))),
            "2 分钟前"
        );
        assert_eq!(
            EN.relative_age(Some(now - Duration::from_secs(120))),
            "2 minutes ago"
        );
        assert_eq!(
            EN.relative_age(Some(now - Duration::from_secs(3_600))),
            "1 hour ago"
        );
        assert_eq!(ZH.relative_age(None), "未知");
        assert_eq!(EN.relative_age(None), "Unknown");
    }
}
