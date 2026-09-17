//! 사용자에게 보일 문구.
//!
//! **완성된 문장이 아니라 키와 인자를 들고 다닙니다.** 문장을 만드는 일은
//! 화면에 내보내는 쪽(CLI 또는 편집기)이 그때의 언어로 합니다.
//!
//! 검증기나 배포기가 한국어 문장을 바로 만들면, 편집기 화면을 영어로 바꿔도
//! 오류만 한국어로 남습니다. 그리고 편집기가 오류 종류를 알아내려고 한국어
//! 문구를 문자열 비교하게 되는데, 문구를 다듬는 순간 조용히 깨집니다.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::i18n::Strings;

#[derive(Debug, Clone, Serialize)]
pub struct Message {
    pub key: &'static str,
    /// `{이름}` 자리에 끼워 넣을 값들.
    pub args: BTreeMap<&'static str, String>,
}

impl Message {
    pub fn new(key: &'static str) -> Message {
        Message {
            key,
            args: BTreeMap::new(),
        }
    }

    pub fn with(mut self, name: &'static str, value: impl ToString) -> Message {
        self.args.insert(name, value.to_string());
        self
    }

    /// 인자 자체가 번역되어야 할 때. 예: "푸시 실패" 의 "푸시".
    ///
    /// 값 앞에 `@` 를 붙여 두고 조립할 때 한 번 더 찾아봅니다. 인자를 그 자리에서
    /// 번역해 넣으면 `Message` 가 언어를 정해버리게 되는데, 그러면 CLI 와 편집기가
    /// 서로 다른 언어를 골라 쓰는 지금 구조가 무너집니다.
    pub fn with_key(self, name: &'static str, key: &str) -> Message {
        self.with(name, format!("@{key}"))
    }

    /// 주어진 언어로 문장을 만듭니다.
    ///
    /// 없는 키는 키 자체가 나옵니다 — 화면에 `msg.something` 이 보이면 번역이
    /// 빠졌다는 뜻이고, 빈 칸보다 훨씬 찾기 쉽습니다.
    pub fn render(&self, strings: &Strings) -> String {
        let mut text = strings.get(self.key).to_string();
        for (name, value) in &self.args {
            let value = match value.strip_prefix('@') {
                Some(key) => strings.get(key).to_string(),
                None => value.clone(),
            };
            text = text.replace(&format!("{{{name}}}"), &value);
        }
        text
    }
}

/// 키 하나로 끝나는 문구를 짧게 쓰기 위한 것.
impl From<&'static str> for Message {
    fn from(key: &'static str) -> Message {
        Message::new(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn fills_placeholders() {
        let strings = Strings::load("ko", Path::new(".")).unwrap();
        let message = Message::new("msg.color.invalid").with("value", "#nothex");

        let rendered = message.render(&strings);
        assert!(rendered.contains("#nothex"), "{rendered}");
        assert!(!rendered.contains("{value}"), "{rendered}");
    }

    /// 같은 문구가 언어마다 다르게 나와야 합니다.
    #[test]
    fn renders_in_each_language() {
        let message = Message::new("msg.color.invalid").with("value", "#zz");

        let mut seen = Vec::new();
        for lang in ["ko", "en", "ja"] {
            let strings = Strings::load(lang, Path::new(".")).unwrap();
            let rendered = message.render(&strings);
            assert!(rendered.contains("#zz"), "{lang}: {rendered}");
            assert!(!rendered.starts_with("msg."), "{lang} 번역이 없습니다");
            seen.push(rendered);
        }
        assert_ne!(seen[0], seen[1], "한국어와 영어가 같습니다");
    }

    #[test]
    fn unknown_key_shows_itself() {
        let strings = Strings::load("ko", Path::new(".")).unwrap();
        assert_eq!(Message::new("msg.nope").render(&strings), "msg.nope");
    }
}
