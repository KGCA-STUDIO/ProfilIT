//! 새 프로필 만들기.
//!
//! 빈 폴더에서 시작할 수 있어야 합니다 — 데스크톱 앱에서 "새 명함"을 고르면
//! 이 함수가 불립니다. 예시 설정을 통째로 복사하지 않고 최소한만 적는 이유는,
//! 처음 여는 사람이 자기 것으로 바꿔야 할 자리를 한눈에 보게 하기 위해서입니다.

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

/// `profile.toml` 을 만듭니다. 이미 있으면 **덮어쓰지 않습니다** — 실수로
/// 내용을 날리는 것이 빈 폴더에서 시작하는 불편보다 훨씬 나쁩니다.
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

    /// 시작 파일이 곧바로 검증을 통과해야 합니다. 새로 만든 프로필이
    /// 오류부터 뱉으면 처음 쓰는 사람이 바로 막힙니다.
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
