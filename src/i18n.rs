//! Internationalization.
//!
//! Two things are kept distinct here.
//!
//! - **Content** — text the user writes. Given per language inside
//!   `profile.toml` ([`Text`]).
//! - **UI strings** — fixed phrases like "Save contact." Live in
//!   `locales/<code>.json` ([`Strings`]).
//!
//! Why content lives inline in the config instead of a separate file: keeping
//! a translation right next to its source text makes it obvious when one is
//! missing. A path-based approach (`sections[1].title`) silently drifts out
//! of sync the moment section order changes.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// A translatable string.
///
/// With only one language, it's just a plain string. Existing config files
/// keep working as-is, and you can upgrade individual entries to a table only
/// where translation is actually needed.
///
/// ```toml
/// title = "학력"                                  # same for every language
/// title = { ko = "학력", en = "Education" }        # per language
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Text {
    Plain(String),
    Translated(BTreeMap<String, String>),
}

impl Text {
    /// The string for a given language. Falls back to `fallback`, then to
    /// whatever's available.
    ///
    /// Showing the source text is better than showing a blank when a
    /// translation is missing. Missing translations are reported as
    /// warnings during validation.
    pub fn get<'a>(&'a self, lang: &str, fallback: &str) -> &'a str {
        match self {
            Text::Plain(value) => value,
            Text::Translated(map) => map
                .get(lang)
                .or_else(|| map.get(fallback))
                .map(String::as_str)
                .or_else(|| map.values().next().map(String::as_str))
                .unwrap_or(""),
        }
    }

    /// Whether an actual translation exists for the given language. Used by
    /// validation to find missing translations.
    pub fn has(&self, lang: &str) -> bool {
        match self {
            // Shared across all languages, so it's present no matter which one is asked for.
            Text::Plain(value) => !value.trim().is_empty(),
            Text::Translated(map) => map.get(lang).is_some_and(|v| !v.trim().is_empty()),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Text::Plain(value) => value.trim().is_empty(),
            Text::Translated(map) => {
                map.is_empty() || map.values().all(|v| v.trim().is_empty())
            }
        }
    }
}

impl From<&str> for Text {
    fn from(value: &str) -> Self {
        Text::Plain(value.to_string())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// UI strings
// ─────────────────────────────────────────────────────────────────────────────

/// Built-in translations. To add a language, create the JSON file and add one line here.
const BUILTIN: &[(&str, &str)] = &[
    ("ko", include_str!("../locales/ko.json")),
    ("en", include_str!("../locales/en.json")),
    ("ja", include_str!("../locales/ja.json")),
];

/// The set of UI strings for one language.
#[derive(Debug, Clone, Default)]
pub struct Strings {
    entries: BTreeMap<String, String>,
    fallback: BTreeMap<String, String>,
}

impl Strings {
    /// Loads the strings for `lang`. If the project has a
    /// `locales/<lang>.json`, it's layered on top of the built-in strings —
    /// so overriding a single string doesn't require copying the whole file.
    pub fn load(lang: &str, project_root: &std::path::Path) -> Result<Strings, String> {
        let mut entries = builtin(lang).unwrap_or_default();

        let path = project_root.join("locales").join(format!("{lang}.json"));
        if path.exists() {
            let source = std::fs::read_to_string(&path)
                .map_err(|e| format!("{} 을 읽을 수 없습니다: {e}", path.display()))?;
            let overrides: BTreeMap<String, String> = serde_json::from_str(&source)
                .map_err(|e| format!("{} 파싱 실패: {e}", path.display()))?;
            entries.extend(overrides);
        }

        if entries.is_empty() {
            return Err(format!(
                "'{lang}' 문구를 찾을 수 없습니다. 내장 언어({})가 아니면 \
                 locales/{lang}.json 을 만들어 주세요.",
                builtin_codes().join(", ")
            ));
        }

        Ok(Strings {
            entries,
            // Safety net for keys that were just added and don't have a translation yet.
            fallback: builtin("en").unwrap_or_default(),
        })
    }

    /// A single string. Falls back to English, then to the key itself —
    /// seeing the raw key on screen makes it obvious what's missing.
    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        self.entries
            .get(key)
            .or_else(|| self.fallback.get(key))
            .map(String::as_str)
            .unwrap_or(key)
    }

    /// Fills in `{name}` placeholders.
    pub fn format(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut text = self.get(key).to_string();
        for (name, value) in args {
            text = text.replace(&format!("{{{name}}}"), value);
        }
        text
    }

    /// Picks out only the strings starting with a given prefix. Lets editor
    /// UI strings (`editor.`) live in the same file as card-facing strings
    /// while only pulling out the ones you need.
    pub fn with_prefix<'a>(&'a self, prefix: &str) -> BTreeMap<&'a str, &'a str> {
        self.entries
            .iter()
            .filter(|(key, _)| key.starts_with(prefix))
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect()
    }

    /// The subset of strings used in the browser. The renderer embeds this as JSON on the page.
    pub fn client_subset(&self) -> BTreeMap<&str, &str> {
        const KEYS: [&str; 4] = [
            "toast.copied",
            "toast.copy_failed",
            "toast.vcard_saved",
            "toast.vcard_failed",
        ];
        KEYS.iter().map(|k| (*k, self.get(k))).collect()
    }
}

fn builtin(lang: &str) -> Option<BTreeMap<String, String>> {
    let source = BUILTIN.iter().find(|(code, _)| *code == lang)?.1;
    // Built-in files are bundled at build time, so if this is malformed it's on us.
    Some(serde_json::from_str(source).expect("내장 locale JSON 이 올바르지 않습니다"))
}

pub fn builtin_codes() -> Vec<&'static str> {
    BUILTIN.iter().map(|(code, _)| *code).collect()
}

/// The name shown in the language switcher. Each language file supplies its
/// own name (`language.name`). Showing a language in its own script is
/// always easier to spot — "日本語" reads better than "Japanese" even on a
/// Korean-language page.
pub fn language_name(lang: &str, project_root: &std::path::Path) -> String {
    Strings::load(lang, project_root)
        .map(|s| s.get("language.name").to_string())
        .unwrap_or_else(|_| lang.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn translated(pairs: &[(&str, &str)]) -> Text {
        Text::Translated(
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        )
    }

    #[test]
    fn plain_text_is_shared_across_languages() {
        let text = Text::from("Rust");
        assert_eq!(text.get("ko", "ko"), "Rust");
        assert_eq!(text.get("en", "ko"), "Rust");
        assert!(text.has("en"));
    }

    #[test]
    fn missing_translation_falls_back_to_default_language() {
        let text = translated(&[("ko", "학력")]);
        assert_eq!(text.get("en", "ko"), "학력");
        assert!(!text.has("en"));
    }

    /// Even if the default language is missing, show whatever exists instead of a blank screen.
    #[test]
    fn falls_back_to_any_available_language() {
        let text = translated(&[("ja", "学歴")]);
        assert_eq!(text.get("en", "ko"), "学歴");
    }

    #[test]
    fn blank_translation_does_not_count_as_present() {
        let text = translated(&[("ko", "학력"), ("en", "   ")]);
        assert!(!text.has("en"));
        assert!(!text.is_empty());
    }

    #[test]
    fn builtin_locales_parse_and_share_keys() {
        let ko = builtin("ko").unwrap();
        for code in builtin_codes() {
            let other = builtin(code).unwrap();
            // If keys drift apart, some language ends up showing the raw key on screen.
            let missing: Vec<_> = ko.keys().filter(|k| !other.contains_key(*k)).collect();
            assert!(missing.is_empty(), "{code} 에 빠진 키: {missing:?}");
        }
    }

    #[test]
    fn format_fills_placeholders() {
        let strings = Strings {
            entries: [("checklist.progress".to_string(), "{total}개 중 {done}개".to_string())]
                .into_iter()
                .collect(),
            fallback: BTreeMap::new(),
        };
        assert_eq!(
            strings.format("checklist.progress", &[("total", "5"), ("done", "2")]),
            "5개 중 2개"
        );
    }

    /// An unknown key should return itself so it's obvious what's missing.
    #[test]
    fn unknown_key_returns_itself() {
        let strings = Strings::default();
        assert_eq!(strings.get("nope.missing"), "nope.missing");
    }
}
