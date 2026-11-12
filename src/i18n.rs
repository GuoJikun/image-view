//! 中/英界面文案。
//!
//! 资源表集中在本模块，用 `Key` 枚举索引；`t()` 与 `tf()` 按当前语言返回文案。
//! 语言是线程局部的：界面只在主线程绘制，测试线程各自设置互不影响。
//! 未覆盖的日志文本、格式名与快捷键不进入资源表。

use std::cell::Cell;
use std::fmt::Display;

use serde::{Deserialize, Serialize};

/// 用户可选的语言。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    /// 跟随系统；非中文系统回退英文。
    #[default]
    System,
    Zh,
    En,
}

impl Language {
    /// 语言菜单中的显示名；`简体中文` / `English` 使用各自语言的原生写法。
    pub fn label(self) -> &'static str {
        match self {
            Self::System => t(Key::LanguageSystem),
            Self::Zh => "简体中文",
            Self::En => "English",
        }
    }
}

/// 实际生效的语言（已解析“跟随系统”）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Zh,
    En,
}

thread_local! {
    static CURRENT: Cell<Lang> = Cell::new(detect_lang());
}

fn detect_lang() -> Lang {
    match sys_locale::get_locale() {
        Some(locale) if locale.to_ascii_lowercase().starts_with("zh") => Lang::Zh,
        _ => Lang::En,
    }
}

/// 应用语言设置；`System` 按系统语言解析。
pub fn set_language(setting: Language) {
    CURRENT.with(|current| current.set(resolve(setting)));
}

/// 当前生效的语言。
pub fn language() -> Lang {
    CURRENT.with(Cell::get)
}

fn resolve(setting: Language) -> Lang {
    match setting {
        Language::System => detect_lang(),
        Language::Zh => Lang::Zh,
        Language::En => Lang::En,
    }
}

/// 资源键。新增文案时同时补齐中英两套 `match`，编译器会检查完整性。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    AppName,
    OpenFilter,
    OpenDialog,
    Close,
    Retry,
    Later,
    // 工具栏
    OpenTooltip,
    PrevTooltip,
    NextTooltip,
    MoreActions,
    SlideshowTooltip,
    ThumbnailsTooltip,
    ZoomOutTooltip,
    ZoomInTooltip,
    FitTooltip,
    SortName,
    SortModified,
    // 溢出菜单与主题
    RevealInExplorer,
    CopyPathMenu,
    MoveToTrashMenu,
    ShowColorStatus,
    ThemeMenu,
    ThemeSystem,
    ThemeLight,
    ThemeDark,
    OpenLogsFolder,
    ShortcutsEllipsis,
    CheckUpdates,
    LanguageMenu,
    LanguageSystem,
    // 状态栏与空状态
    NoImage,
    Downscaled,
    SlideshowRunning,
    DimensionsDownscaled,
    ColorConverted,
    EmptyTitle,
    EmptyHint,
    EmptyOpen,
    // 帮助窗口
    HelpTitle,
    HelpOpen,
    HelpNavigate,
    HelpSlideshow,
    HelpZoom,
    HelpFit,
    HelpToggleFit,
    HelpRotateCw,
    HelpRotateCcw,
    HelpCopyPath,
    HelpTrash,
    HelpFullscreen,
    HelpEscape,
    // 错误与提示
    ErrorNoImagesInDir,
    ErrorFileMissing,
    NoticePathCopied,
    ErrorCreateLogDir,
    NoticeMovedToTrash,
    ErrorTrashFailed,
    ErrorOpenFileManager,
    LoadRead,
    LoadDecode,
    LoadTooLarge,
    LoadOutOfMemory,
    LoadPreloadSkipped,
    LoadAvif,
    LoadHeic,
    LoadSvg,
    LoadMissingDecoder,
    SvgzUnsupported,
    // 自动更新
    UpdateTitle,
    UpdateChecking,
    UpdateUpToDate,
    UpdateAvailable,
    UpdateDownloading,
    UpdateReady,
    UpdateError,
    UpdateCheck,
    UpdateDownload,
    UpdateInstall,
    UpdateNotes,
    UpdateHint,
    UpdateCurrent,
    UpdateStartup,
    UpdateNotSupported,
    UpdateErrNetwork,
    UpdateErrStatus,
    UpdateErrManifest,
    UpdateErrVersion,
    UpdateErrHash,
    UpdateErrIo,
    UpdateErrUrl,
}

/// 取当前语言的文案。
pub fn t(key: Key) -> &'static str {
    match language() {
        Lang::Zh => zh(key),
        Lang::En => en(key),
    }
}

/// 取文案并按顺序填充 `{}` 占位符；多余参数忽略，缺少参数保留 `{}`。
pub fn tf(key: Key, args: &[&dyn Display]) -> String {
    let mut out = String::new();
    let template = t(key);
    let mut rest = template;
    let mut args = args.iter();

    while let Some(index) = rest.find("{}") {
        out.push_str(&rest[..index]);
        match args.next() {
            Some(arg) => out.push_str(&arg.to_string()),
            None => out.push_str("{}"),
        }
        rest = &rest[index + 2..];
    }
    out.push_str(rest);
    out
}

fn zh(key: Key) -> &'static str {
    match key {
        Key::AppName => "图片查看器",
        Key::OpenFilter => "图片",
        Key::OpenDialog => "打开图片",
        Key::Close => "关闭",
        Key::Retry => "重试",
        Key::Later => "稍后",
        Key::OpenTooltip => "打开图片 (Ctrl+O)",
        Key::PrevTooltip => "上一张 (←)",
        Key::NextTooltip => "下一张 (→)",
        Key::MoreActions => "更多操作",
        Key::SlideshowTooltip => "幻灯片 (Space)",
        Key::ThumbnailsTooltip => "显示/隐藏缩略图条",
        Key::ZoomOutTooltip => "缩小 (-)",
        Key::ZoomInTooltip => "放大 (+)",
        Key::FitTooltip => "适配窗口 (0)",
        Key::SortName => "名称",
        Key::SortModified => "修改时间",
        Key::RevealInExplorer => "在资源管理器中显示",
        Key::CopyPathMenu => "复制路径    Ctrl+Shift+C",
        Key::MoveToTrashMenu => "移到回收站    Delete",
        Key::ShowColorStatus => "显示色彩转换标记",
        Key::ThemeMenu => "主题",
        Key::ThemeSystem => "跟随系统",
        Key::ThemeLight => "浅色",
        Key::ThemeDark => "深色",
        Key::OpenLogsFolder => "打开日志文件夹",
        Key::ShortcutsEllipsis => "快捷键…",
        Key::CheckUpdates => "检查更新…",
        Key::LanguageMenu => "语言",
        Key::LanguageSystem => "跟随系统",
        Key::NoImage => "未打开图片",
        Key::Downscaled => "已降采样",
        Key::SlideshowRunning => "幻灯片中",
        Key::DimensionsDownscaled => "{}×{}（显示 {}×{}）",
        Key::ColorConverted => "已转换到 sRGB",
        Key::EmptyTitle => "还没有打开图片",
        Key::EmptyHint => "拖入文件，或按 Ctrl+O 选择",
        Key::EmptyOpen => "打开图片…",
        Key::HelpTitle => "快捷键",
        Key::HelpOpen => "打开文件",
        Key::HelpNavigate => "上一张 / 下一张",
        Key::HelpSlideshow => "开始 / 停止幻灯片",
        Key::HelpZoom => "放大 / 缩小",
        Key::HelpFit => "适配窗口",
        Key::HelpToggleFit => "切换适配 / 100%",
        Key::HelpRotateCw => "顺时针旋转 90°",
        Key::HelpRotateCcw => "逆时针旋转 90°",
        Key::HelpCopyPath => "复制当前图片路径",
        Key::HelpTrash => "移到回收站",
        Key::HelpFullscreen => "全屏",
        Key::HelpEscape => "退出全屏 / 关闭帮助",
        Key::ErrorNoImagesInDir => "目录中没有支持的图片：{}",
        Key::ErrorFileMissing => "文件不存在：{}",
        Key::NoticePathCopied => "已复制路径",
        Key::ErrorCreateLogDir => "无法创建日志目录 {}：{}",
        Key::NoticeMovedToTrash => "已移到回收站：{}",
        Key::ErrorTrashFailed => "移到回收站失败：{}",
        Key::ErrorOpenFileManager => "无法打开文件管理器：{}",
        Key::LoadRead => "无法读取文件：{}",
        Key::LoadDecode => "图片已损坏或格式不受支持：{}",
        Key::LoadTooLarge => "图片过大（{}×{}），已超过安全上限",
        Key::LoadOutOfMemory => "解码超出内存限制",
        Key::LoadPreloadSkipped => "预加载已跳过（原图 {}×{} 过大）",
        Key::LoadAvif => "AVIF 解码失败：{}",
        Key::LoadHeic => "HEIC 解码失败：{}",
        Key::LoadSvg => "SVG 解析失败：{}",
        Key::LoadMissingDecoder => "{} 解码器未包含在此构建中；请使用启用该特性的版本打开 .{} 文件",
        Key::SvgzUnsupported => "暂不支持 gzip 压缩的 SVGZ，请解压为 .svg 后再打开",
        Key::UpdateTitle => "软件更新",
        Key::UpdateChecking => "正在检查更新…",
        Key::UpdateUpToDate => "已是最新版本",
        Key::UpdateAvailable => "发现新版本 {}（当前 {}）",
        Key::UpdateDownloading => "正在下载更新…",
        Key::UpdateReady => "更新已下载并通过校验，可以安装",
        Key::UpdateError => "更新失败：{}",
        Key::UpdateCheck => "检查更新",
        Key::UpdateDownload => "下载",
        Key::UpdateInstall => "安装并退出",
        Key::UpdateNotes => "发布说明",
        Key::UpdateHint => "检查更新需要联网；安装前会校验 SHA-256。",
        Key::UpdateCurrent => "当前版本：{}",
        Key::UpdateStartup => "启动时检查更新",
        Key::UpdateNotSupported => "当前平台不支持自动安装，请从发布页手动下载",
        Key::UpdateErrNetwork => "无法连接更新服务器：{}",
        Key::UpdateErrStatus => "更新服务器返回状态 {}",
        Key::UpdateErrManifest => "更新清单无效：{}",
        Key::UpdateErrVersion => "版本号无效：{}",
        Key::UpdateErrHash => "安装包校验失败（SHA-256 不匹配）",
        Key::UpdateErrIo => "文件操作失败：{}",
        Key::UpdateErrUrl => "更新下载地址无效",
    }
}

fn en(key: Key) -> &'static str {
    match key {
        Key::AppName => "Image Viewer",
        Key::OpenFilter => "Images",
        Key::OpenDialog => "Open Image",
        Key::Close => "Close",
        Key::Retry => "Retry",
        Key::Later => "Later",
        Key::OpenTooltip => "Open image (Ctrl+O)",
        Key::PrevTooltip => "Previous (←)",
        Key::NextTooltip => "Next (→)",
        Key::MoreActions => "More actions",
        Key::SlideshowTooltip => "Slideshow (Space)",
        Key::ThumbnailsTooltip => "Show/hide thumbnails",
        Key::ZoomOutTooltip => "Zoom out (-)",
        Key::ZoomInTooltip => "Zoom in (+)",
        Key::FitTooltip => "Fit to window (0)",
        Key::SortName => "Name",
        Key::SortModified => "Date modified",
        Key::RevealInExplorer => "Show in File Explorer",
        Key::CopyPathMenu => "Copy path    Ctrl+Shift+C",
        Key::MoveToTrashMenu => "Move to Recycle Bin    Delete",
        Key::ShowColorStatus => "Show color conversion badge",
        Key::ThemeMenu => "Theme",
        Key::ThemeSystem => "Follow system",
        Key::ThemeLight => "Light",
        Key::ThemeDark => "Dark",
        Key::OpenLogsFolder => "Open log folder",
        Key::ShortcutsEllipsis => "Keyboard shortcuts…",
        Key::CheckUpdates => "Check for updates…",
        Key::LanguageMenu => "Language",
        Key::LanguageSystem => "Follow system",
        Key::NoImage => "No image open",
        Key::Downscaled => "Downscaled",
        Key::SlideshowRunning => "Slideshow",
        Key::DimensionsDownscaled => "{}×{} (display {}×{})",
        Key::ColorConverted => "Converted to sRGB",
        Key::EmptyTitle => "No image open",
        Key::EmptyHint => "Drop a file here, or press Ctrl+O",
        Key::EmptyOpen => "Open image…",
        Key::HelpTitle => "Keyboard shortcuts",
        Key::HelpOpen => "Open file",
        Key::HelpNavigate => "Previous / next image",
        Key::HelpSlideshow => "Start / stop slideshow",
        Key::HelpZoom => "Zoom in / out",
        Key::HelpFit => "Fit to window",
        Key::HelpToggleFit => "Toggle fit / 100%",
        Key::HelpRotateCw => "Rotate 90° clockwise",
        Key::HelpRotateCcw => "Rotate 90° counter-clockwise",
        Key::HelpCopyPath => "Copy current image path",
        Key::HelpTrash => "Move to Recycle Bin",
        Key::HelpFullscreen => "Fullscreen",
        Key::HelpEscape => "Exit fullscreen / close help",
        Key::ErrorNoImagesInDir => "No supported images in this folder: {}",
        Key::ErrorFileMissing => "File not found: {}",
        Key::NoticePathCopied => "Path copied",
        Key::ErrorCreateLogDir => "Failed to create log folder {}: {}",
        Key::NoticeMovedToTrash => "Moved to Recycle Bin: {}",
        Key::ErrorTrashFailed => "Failed to move to Recycle Bin: {}",
        Key::ErrorOpenFileManager => "Failed to open File Explorer: {}",
        Key::LoadRead => "Failed to read file: {}",
        Key::LoadDecode => "Image is corrupt or the format is unsupported: {}",
        Key::LoadTooLarge => "Image too large ({}×{}), exceeds the safety limit",
        Key::LoadOutOfMemory => "Decoding exceeded the memory limit",
        Key::LoadPreloadSkipped => "Preload skipped (source {}×{} is too large)",
        Key::LoadAvif => "AVIF decoding failed: {}",
        Key::LoadHeic => "HEIC decoding failed: {}",
        Key::LoadSvg => "SVG parsing failed: {}",
        Key::LoadMissingDecoder => {
            "{} decoder is not included in this build; use a build with that feature to open .{} files"
        }
        Key::SvgzUnsupported => "Gzip-compressed SVGZ is not supported; extract it to .svg first",
        Key::UpdateTitle => "Software Update",
        Key::UpdateChecking => "Checking for updates…",
        Key::UpdateUpToDate => "You're up to date",
        Key::UpdateAvailable => "Version {} is available (current {})",
        Key::UpdateDownloading => "Downloading update…",
        Key::UpdateReady => "Update downloaded and verified; ready to install",
        Key::UpdateError => "Update failed: {}",
        Key::UpdateCheck => "Check for updates",
        Key::UpdateDownload => "Download",
        Key::UpdateInstall => "Install and exit",
        Key::UpdateNotes => "Release notes",
        Key::UpdateHint => {
            "Checking for updates requires an internet connection; SHA-256 is verified before installation."
        }
        Key::UpdateCurrent => "Current version: {}",
        Key::UpdateStartup => "Check for updates at startup",
        Key::UpdateNotSupported => {
            "Automatic installation is not supported on this platform; download from the release page"
        }
        Key::UpdateErrNetwork => "Could not reach the update server: {}",
        Key::UpdateErrStatus => "Update server returned status {}",
        Key::UpdateErrManifest => "Invalid update manifest: {}",
        Key::UpdateErrVersion => "Invalid version: {}",
        Key::UpdateErrHash => "Installer verification failed (SHA-256 mismatch)",
        Key::UpdateErrIo => "File operation failed: {}",
        Key::UpdateErrUrl => "Invalid update download URL",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_placeholders_in_order() {
        set_language(Language::Zh);
        assert_eq!(tf(Key::ErrorFileMissing, &[&"a.png"]), "文件不存在：a.png");

        set_language(Language::En);
        assert_eq!(
            tf(Key::ErrorFileMissing, &[&"a.png"]),
            "File not found: a.png"
        );
    }

    #[test]
    fn missing_arguments_keep_placeholder() {
        set_language(Language::En);
        assert_eq!(
            tf(Key::LoadTooLarge, &[&1920]),
            "Image too large (1920×{}), exceeds the safety limit"
        );
    }

    #[test]
    fn explicit_language_overrides_system() {
        set_language(Language::En);
        assert_eq!(t(Key::AppName), "Image Viewer");
        set_language(Language::Zh);
        assert_eq!(t(Key::AppName), "图片查看器");
    }

    #[test]
    fn language_labels_are_native() {
        set_language(Language::En);
        assert_eq!(Language::Zh.label(), "简体中文");
        assert_eq!(Language::En.label(), "English");
        assert_eq!(Language::System.label(), "Follow system");

        set_language(Language::Zh);
        assert_eq!(Language::System.label(), "跟随系统");
    }
}
