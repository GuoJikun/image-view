//! 原生菜单（macOS）文案。
//!
//! 直接复用前端文案表 `app/src/locales/*.json`：构建时 `include_str!` 内嵌，运行时按点号路径取值，
//! 不再在 Rust 里另写一份中英对照。改文案只改 JSON，两端自动一致。
//!
//! 注意 JSON 里的消息语法（`{name}` 插值、`a | b` 复数）只有前端 vue-i18n 认识；
//! 菜单项都是纯文本，若将来给菜单键加了占位符，需要在这里先渲染再交给菜单。

#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use std::sync::OnceLock;

use serde_json::Value;

/// 菜单用到的键，顺序同菜单自上而下；新增菜单项时在这里补键。
pub const KEYS: &[&str] = &[
    // 应用菜单
    "app.name",
    // 文件
    "menu.file",
    "menu.open",
    "menu.reveal",
    "menu.copyPath",
    "menu.trash",
    // 显示 › 排序
    "menu.view",
    "menu.thumbnails",
    "menu.slideshow",
    "menu.sort",
    "menu.sortName",
    "menu.sortModified",
    // 显示 › 缩放
    "menu.zoomIn",
    "menu.zoomOut",
    "menu.fit",
    "menu.actual",
    // 显示 › 旋转 / 翻转
    "menu.rotateCW",
    "menu.rotateCCW",
    "menu.flipH",
    "menu.flipV",
    // 显示 › 主题
    "menu.theme",
    "menu.themeSystem",
    "menu.themeLight",
    "menu.themeDark",
    // 显示 › 语言
    "menu.language",
    "menu.langSystem",
    "menu.langZh",
    "menu.langEn",
    // 显示 › 其它
    "menu.colorBadge",
    "menu.metadata",
    // 帮助
    "menu.help",
    "menu.shortcuts",
    "menu.update",
    "menu.autoUpdate",
    "menu.logs",
];

const ZH_JSON: &str = include_str!("../../src/locales/zh.json");
const EN_JSON: &str = include_str!("../../src/locales/en.json");

static ZH: OnceLock<Value> = OnceLock::new();
static EN: OnceLock<Value> = OnceLock::new();

fn parse(source: &str, locale: &str) -> Value {
    serde_json::from_str(source).unwrap_or_else(|error| {
        log::error!("文案表 locales/{locale}.json 无法解析：{error}");
        Value::Null
    })
}

fn tree(locale: &str) -> &'static Value {
    if locale.starts_with("en") {
        EN.get_or_init(|| parse(EN_JSON, "en"))
    } else {
        ZH.get_or_init(|| parse(ZH_JSON, "zh"))
    }
}

/// 某种语言的菜单文案。JSON 只解析一次，之后取词全部返回 `'static` 引用。
pub struct MenuText {
    tree: &'static Value,
}

impl MenuText {
    pub fn new(locale: &str) -> Self {
        Self { tree: tree(locale) }
    }

    /// 按点号路径取文案；键缺失时退回键名并记一条警告（测试保证正式键都存在）。
    pub fn t(&self, key: &'static str) -> &'static str {
        let mut node = self.tree;
        for part in key.split('.') {
            match node.get(part) {
                Some(next) => node = next,
                None => {
                    log::warn!("菜单文案缺失：{key}");
                    return key;
                }
            }
        }
        node.as_str().unwrap_or_else(|| {
            log::warn!("菜单文案不是字符串：{key}");
            key
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 两种语言都要能取到每个菜单键，否则 macOS 菜单会显示成键名。
    #[test]
    fn menu_keys_resolve_in_both_locales() {
        for locale in ["zh", "en"] {
            let text = MenuText::new(locale);
            for key in KEYS {
                let value = text.t(key);
                assert!(
                    !value.is_empty() && value != *key,
                    "{locale} 取不到菜单文案 {key}（拿到 {value:?}）"
                );
            }
        }
    }

    /// 中英文菜单文案不应完全相同（`简体中文` / `English` 这类专有名称除外）。
    #[test]
    fn english_and_chinese_differ_for_translatable_keys() {
        const SAME_BY_DESIGN: &[&str] = &["menu.langZh", "menu.langEn"];
        let zh = MenuText::new("zh");
        let en = MenuText::new("en");
        for key in KEYS.iter().filter(|key| !SAME_BY_DESIGN.contains(key)) {
            assert_ne!(zh.t(key), en.t(key), "{key} 的中英文案相同，可能漏翻");
        }
    }

    #[test]
    fn missing_key_falls_back_to_key_name() {
        assert_eq!(MenuText::new("zh").t("menu.不存在"), "menu.不存在");
    }

    /// 未识别的语言标签按中文处理（与前端 `resolvedLocale` 的回退方向一致）。
    #[test]
    fn unknown_locale_falls_back_to_chinese() {
        assert_eq!(
            MenuText::new("fr").t("menu.file"),
            MenuText::new("zh").t("menu.file")
        );
    }
}
