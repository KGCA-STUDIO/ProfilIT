//! 다국어.
//!
//! 두 가지를 구분합니다.
//!
//! - **내용** — 사용자가 쓴 글. `profile.toml` 안에 언어별로 적습니다([`Text`]).
//! - **UI 문구** — "연락처 저장" 같은 고정 문구. `locales/<코드>.json` 에 있습니다([`Strings`]).
//!
//! 내용을 별도 파일로 빼지 않고 설정 안에 두는 이유: 번역이 원문 바로 옆에
//! 있어야 빠뜨린 항목이 눈에 띕니다. 경로로 연결하는 방식(`sections[1].title`)은
//! 섹션 순서만 바꿔도 조용히 어긋납니다.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// 번역 가능한 문자열.
///
/// 한 언어만 쓰면 평범한 문자열 그대로입니다. 기존 설정 파일이 그대로 읽히고,
/// 번역이 필요한 항목만 골라서 표로 바꾸면 됩니다.
///
/// ```toml
/// title = "학력"                                  # 모든 언어 공통
/// title = { ko = "학력", en = "Education" }        # 언어별
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Text {
    Plain(String),
    Translated(BTreeMap<String, String>),
}

impl Text {
    /// 해당 언어의 문자열. 없으면 `fallback` 언어, 그것도 없으면 아무거나.
    ///
    /// 번역이 빠졌다고 빈 화면을 보여주는 것보다 원문이라도 보여주는 쪽이
    /// 낫습니다. 빠진 번역은 검증에서 경고로 알립니다.
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

    /// 해당 언어로 실제 번역이 있는지. 검증에서 빠진 번역을 찾을 때 씁니다.
    pub fn has(&self, lang: &str) -> bool {
        match self {
            // 모든 언어 공통이므로 어떤 언어로 물어도 있습니다.
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
// UI 문구
// ─────────────────────────────────────────────────────────────────────────────

/// 내장 번역. 언어를 추가하려면 JSON 파일을 만들고 여기 한 줄 넣으면 됩니다.
const BUILTIN: &[(&str, &str)] = &[
    ("ko", include_str!("../locales/ko.json")),
    ("en", include_str!("../locales/en.json")),
    ("ja", include_str!("../locales/ja.json")),
];

/// 한 언어의 UI 문구 모음.
#[derive(Debug, Clone, Default)]
pub struct Strings {
    entries: BTreeMap<String, String>,
    fallback: BTreeMap<String, String>,
}

impl Strings {
    /// `lang` 의 문구를 불러옵니다. 프로젝트의 `locales/<lang>.json` 이 있으면
    /// 내장 문구 위에 덮어씁니다 — 문구 하나만 바꾸려고 파일 전체를 적을
    /// 필요가 없습니다.
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
            // 새로 추가된 키가 아직 번역되지 않았을 때를 위한 그물.
            fallback: builtin("en").unwrap_or_default(),
        })
    }

    /// 문구 하나. 없으면 영어, 그것도 없으면 키 자체를 돌려줍니다 —
    /// 화면에 키가 보이면 무엇이 빠졌는지 바로 알 수 있습니다.
    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        self.entries
            .get(key)
            .or_else(|| self.fallback.get(key))
            .map(String::as_str)
            .unwrap_or(key)
    }

    /// `{이름}` 자리를 채웁니다.
    pub fn format(&self, key: &str, args: &[(&str, &str)]) -> String {
        let mut text = self.get(key).to_string();
        for (name, value) in args {
            text = text.replace(&format!("{{{name}}}"), value);
        }
        text
    }

    /// 접두사로 시작하는 문구만 추립니다. 편집기 UI 문구(`editor.`)를
    /// 명함 문구와 한 파일에 두되 필요한 쪽만 꺼내 쓰기 위한 것입니다.
    pub fn with_prefix<'a>(&'a self, prefix: &str) -> BTreeMap<&'a str, &'a str> {
        self.entries
            .iter()
            .filter(|(key, _)| key.starts_with(prefix))
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect()
    }

    /// 브라우저에서 쓰는 문구만 추린 것. 렌더러가 JSON 으로 페이지에 넣습니다.
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
    // 내장 파일은 빌드 시점에 포함되므로 깨져 있으면 우리 잘못입니다.
    Some(serde_json::from_str(source).expect("내장 locale JSON 이 올바르지 않습니다"))
}

pub fn builtin_codes() -> Vec<&'static str> {
    BUILTIN.iter().map(|(code, _)| *code).collect()
}

/// 언어 선택기에 보일 이름. 각 언어 파일이 자기 이름을 직접 적습니다
/// (`language.name`). 선택기에는 늘 그 언어 자신의 표기로 보이는 편이
/// 찾기 쉽습니다 — 한국어 화면이어도 "일본어" 보다 "日本語" 가 낫습니다.
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

    /// 기본 언어조차 없으면 빈 화면 대신 있는 것이라도 보여줍니다.
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
            // 키가 어긋나면 어떤 언어에서는 화면에 키가 그대로 보입니다.
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

    /// 없는 키는 키 자체가 보여야 무엇이 빠졌는지 알 수 있습니다.
    #[test]
    fn unknown_key_returns_itself() {
        let strings = Strings::default();
        assert_eq!(strings.get("nope.missing"), "nope.missing");
    }
}
