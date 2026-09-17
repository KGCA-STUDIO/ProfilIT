//! Creating a new profile.
//!
//! This needs to work starting from an empty folder — picking "New card" in
//! the desktop app calls this function. We write a minimal config rather than
//! copying a full example so that someone opening it for the first time can
//! see at a glance exactly what needs to be filled in.

use std::io;
use std::path::Path;

const STARTER: &str = r##"# ProfileIT 프로필 설정 — 온라인 명함
#
# `cargo run -- edit` 로 편집 UI 를 열거나 이 파일을 직접 고치면 됩니다.
# 필드 목록은 docs/schema.md 를 보세요.

schema_version = 1

[site]
title = "내 온라인 명함"
# 배포 주소. 공유 카드(og:url)를 만들려면 필요합니다.
# base_url = "https://example.github.io/card"
lang = "ko"

[profile]
name = "이름"
tagline = "한 줄 소개"
# location = "서울"
# avatar = "assets/avatar.jpg"

# ── 섹션 ──────────────────────────────────────────────────────
# 배열 순서가 화면 순서입니다. 빼고 싶으면 지우지 말고 enabled = false.

[[sections]]
type = "about"
title = "소개"
icon = "👋"
body = """
여기에 자기소개를 적습니다.

빈 줄로 문단을 나눕니다.
"""

[[sections]]
type = "timeline"
title = "경력"
icon = "💼"

[[sections.items]]
period = "2024 – 현재"
title = "회사 이름"
subtitle = "맡은 일"

[[sections]]
type = "tags"
title = "관심사"
icon = "🧩"
items = ["관심사", { icon = "🍳", text = "이모지도 됩니다" }]

[[sections]]
type = "contact"
title = "연락처"
icon = "📮"

[[sections.items]]
kind = "email"
value = "me@example.com"

# ── 테마 ──────────────────────────────────────────────────────
[theme]
accent = "#2f9fd0"

[theme.background]
type = "gradient"
from = "#dff2fb"
to = "#bfe6f7"
angle = 165

[theme.decoration]
type = "preset"
name = "confetti"

[theme.text]
heading = "#14425c"
body = "#3c5a6b"
card_title = "#1a6f96"
muted = "#7b96a5"

[theme.font]
preset = "pretendard"
heading_weight = 800

# ── GitHub Pages 배포 ─────────────────────────────────────────
# `profileit deploy` 또는 편집기의 "GitHub 배포" 버튼이 dist/ 를
# 아래 브랜치로 올립니다. git 자격증명을 그대로 쓰므로 토큰은
# 필요 없습니다.
[deploy]
remote = "origin"
branch = "gh-pages"
"##;

/// Creates `profile.toml`. If it already exists, this **does not overwrite
/// it** — accidentally wiping someone's content is far worse than the
/// inconvenience of having to start from an empty folder.
pub fn init(path: &Path) -> io::Result<()> {
    if path.exists() {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{} 이 이미 있습니다.", path.display()),
        ));
    }

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }

    std::fs::write(path, STARTER)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The starter file must pass validation right out of the box. If a
    /// freshly created profile immediately throws errors, first-time users
    /// get stuck before they even start.
    #[test]
    fn starter_config_is_valid() {
        let config: crate::config::Config =
            toml::from_str(STARTER).expect("시작 설정 파싱 실패");

        let diagnostics = crate::validate::validate(&config, Path::new("."));
        let errors: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.severity == crate::validate::Severity::Error)
            .collect();

        assert!(errors.is_empty(), "시작 설정에 오류가 있습니다: {errors:#?}");
    }

    #[test]
    fn starter_config_renders() {
        let config: crate::config::Config = toml::from_str(STARTER).unwrap();
        let strings = crate::i18n::Strings::load("ko", Path::new(".")).unwrap();
        let ctx = crate::render::Ctx {
            config: &config,
            lang: "ko",
            fallback: "ko",
            strings: &strings,
            languages: &[],
            prefix: "",
        };
        let html = crate::render::page(&ctx);
        assert!(html.contains("내 온라인 명함"));
    }
}
