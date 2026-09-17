//! User-facing text.
//!
//! **These carry a key and arguments, not a finished sentence.** Building the
//! actual sentence is left to whatever renders it (the CLI or the editor),
//! in whatever language is active at that moment.
//!
//! If the validator or deployer baked in Korean sentences directly, switching
//! the editor UI to English would still leave errors in Korean. Worse, the
//! editor would end up string-matching on those Korean phrases to figure out
//! the error kind, which breaks silently the moment someone tweaks the wording.

use std::collections::BTreeMap;

use serde::Serialize;

use crate::i18n::Strings;

#[derive(Debug, Clone, Serialize)]
pub struct Message {
    pub key: &'static str,
    /// Values to substitute into `{name}` placeholders.
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

    /// For when the argument itself needs translating, e.g. "push" in "push failed".
    ///
    /// We prefix the value with `@` and look it up again at render time. If we
    /// translated the argument right here instead, `Message` would end up locked
    /// to one language, which breaks the whole point of letting the CLI and the
    /// editor each pick their own language.
    pub fn with_key(self, name: &'static str, key: &str) -> Message {
        self.with(name, format!("@{key}"))
    }

    /// Renders the sentence in the given language.
    ///
    /// A missing key falls back to the key itself — if `msg.something` shows up
    /// on screen, that means the translation is missing, and it's much easier to
    /// spot than a blank string.
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

/// Shorthand for messages that are just a single key with no arguments.
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

    /// The same message should render differently per language.
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
