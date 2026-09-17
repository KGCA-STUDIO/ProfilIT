//! 내장 SVG 아이콘.
//!
//! 모든 아이콘은 `viewBox="0 0 24 24"` 에 맞춰져 있고 `currentColor` 를 씁니다.
//! 색은 CSS 토큰이 정하므로 여기에는 고정 색이 없습니다.
//!
//! 플랫폼을 추가하려면 `Platform` 에 변형을 넣고 `platform` 함수에 한 갈래를
//! 더하면 됩니다. 기여 받기 가장 쉬운 지점이라 일부러 단순하게 두었습니다.

use maud::{Markup, PreEscaped};

use crate::config::Platform;

/// `<svg>` 안에 들어갈 내용. 바깥 `<svg>` 태그는 호출하는 쪽이 씁니다.
pub fn platform(platform: Platform) -> Markup {
    let body = match platform {
        Platform::Instagram => {
            r#"<rect x="2.75" y="2.75" width="18.5" height="18.5" rx="5.25" fill="none" stroke="currentColor" stroke-width="1.9"/><circle cx="12" cy="12" r="4.35" fill="none" stroke="currentColor" stroke-width="1.9"/><circle cx="17.4" cy="6.6" r="1.35" fill="currentColor"/>"#
        }
        Platform::Youtube => {
            r#"<path fill="currentColor" d="M21.6 7.2a2.5 2.5 0 0 0-1.76-1.77C18.25 5 12 5 12 5s-6.25 0-7.84.43A2.5 2.5 0 0 0 2.4 7.2 26 26 0 0 0 2 12a26 26 0 0 0 .4 4.8 2.5 2.5 0 0 0 1.76 1.77C5.75 19 12 19 12 19s6.25 0 7.84-.43a2.5 2.5 0 0 0 1.76-1.77A26 26 0 0 0 22 12a26 26 0 0 0-.4-4.8ZM10 15.02V8.98L15.2 12Z"/>"#
        }
        Platform::Threads => {
            r#"<path fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" d="M16.3 11.4c-.3-3-2-4.4-4.3-4.4-1.8 0-3 .8-3.6 2m7.9 2.4c2 .7 3 2 3 3.7 0 2.3-2 3.9-5 3.9-4.3 0-7-2.9-7-7.5S9.7 4 13.6 4c2.3 0 4 .7 5.2 2m-2.5 5.4c-.9-.3-2-.5-3.2-.5-2 0-3.3.9-3.3 2.2s1 2 2.4 2c1.9 0 3.2-1.2 3.4-3.7Z"/>"#
        }
        Platform::Tiktok => {
            r#"<path fill="currentColor" d="M16.5 3h-2.7v12.1a2.4 2.4 0 1 1-2.4-2.4c.2 0 .4 0 .6.1V10a5.2 5.2 0 1 0 4.5 5.1V8.9c1 .7 2.2 1.1 3.5 1.2V7.4a4 4 0 0 1-3.5-4.4Z"/>"#
        }
        Platform::X => {
            r#"<path fill="currentColor" d="M17.5 3h3.2l-7 8 8.2 10h-6.4l-5-6.1L4.7 21H1.5l7.5-8.6L1.2 3h6.6l4.5 5.6Zm-1.1 16.1h1.8L7.7 4.8H5.8Z"/>"#
        }
        Platform::Facebook => {
            r#"<path fill="currentColor" d="M22 12a10 10 0 1 0-11.6 9.9v-7H7.9V12h2.5V9.8c0-2.5 1.5-3.9 3.8-3.9 1.1 0 2.2.2 2.2.2v2.5h-1.3c-1.2 0-1.6.8-1.6 1.6V12h2.8l-.5 2.9h-2.3v7A10 10 0 0 0 22 12Z"/>"#
        }
        // 네이버의 N 자형.
        Platform::Naver | Platform::NaverBlog => {
            r#"<path fill="currentColor" d="M4 4h5.1l5.4 8.1V4H20v16h-5.1L9.5 11.9V20H4Z"/>"#
        }
        Platform::KakaoTalk => {
            r#"<path fill="currentColor" d="M12 3C6.9 3 2.8 6.3 2.8 10.3c0 2.6 1.7 4.8 4.3 6.1l-1 3.8c-.1.3.2.6.5.4l4.5-3c.3 0 .6.1.9.1 5.1 0 9.2-3.3 9.2-7.4S17.1 3 12 3Z"/>"#
        }
        Platform::Github => {
            r#"<path fill="currentColor" d="M12 2a10 10 0 0 0-3.16 19.49c.5.09.68-.22.68-.48v-1.69c-2.78.6-3.37-1.34-3.37-1.34-.45-1.16-1.11-1.47-1.11-1.47-.91-.62.07-.6.07-.6 1 .07 1.53 1.03 1.53 1.03.89 1.53 2.34 1.09 2.91.83.09-.65.35-1.09.63-1.34-2.22-.25-4.55-1.11-4.55-4.94 0-1.09.39-1.98 1.03-2.68-.1-.25-.45-1.27.1-2.65 0 0 .84-.27 2.75 1.02a9.6 9.6 0 0 1 5 0c1.91-1.29 2.75-1.02 2.75-1.02.55 1.38.2 2.4.1 2.65.64.7 1.03 1.59 1.03 2.68 0 3.84-2.34 4.68-4.57 4.93.36.31.68.92.68 1.85v2.74c0 .27.18.58.69.48A10 10 0 0 0 12 2Z"/>"#
        }
        Platform::Linkedin => {
            r#"<path fill="currentColor" d="M4.98 3.5a2.5 2.5 0 1 0 0 5 2.5 2.5 0 0 0 0-5ZM3 9h4v12H3Zm7 0h3.8v1.7h.05c.53-1 1.83-2.05 3.75-2.05C21.6 8.65 22 11.3 22 14.4V21h-4v-5.9c0-1.4 0-3.2-2-3.2s-2.3 1.5-2.3 3.1V21h-4Z"/>"#
        }
        Platform::Email => {
            r#"<rect x="2.8" y="5" width="18.4" height="14" rx="2.6" fill="none" stroke="currentColor" stroke-width="1.8"/><path d="m4 7.5 8 5.2 8-5.2" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>"#
        }
        Platform::Rss => {
            r#"<circle cx="6" cy="18" r="2" fill="currentColor"/><path d="M4 11a9 9 0 0 1 9 9M4 4a16 16 0 0 1 16 16" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"/>"#
        }
        // 검증에서 icon 을 요구하므로 렌더러는 이 갈래를 쓰지 않습니다.
        Platform::Custom => {
            r#"<path d="M9 15 15 9m-4.5-1.5 1.8-1.8a3.9 3.9 0 0 1 5.5 5.5l-1.8 1.8m-4.5 4.5-1.8 1.8a3.9 3.9 0 0 1-5.5-5.5l1.8-1.8" fill="none" stroke="currentColor" stroke-width="1.9" stroke-linecap="round"/>"#
        }
    };

    PreEscaped(body.to_string())
}

/// 카드 오른쪽 공유 버튼의 점 세 개.
pub fn share() -> Markup {
    PreEscaped(
        r#"<circle cx="12" cy="5" r="1.9" fill="currentColor"/><circle cx="12" cy="12" r="1.9" fill="currentColor"/><circle cx="12" cy="19" r="1.9" fill="currentColor"/>"#
            .to_string(),
    )
}

/// 프로필 지역 표기 앞의 핀.
pub fn location() -> Markup {
    PreEscaped(
        r#"<path d="M12 21s7-6.2 7-11a7 7 0 1 0-14 0c0 4.8 7 11 7 11Z" fill="none" stroke="currentColor" stroke-width="1.8"/><circle cx="12" cy="10" r="2.6" fill="currentColor"/>"#
            .to_string(),
    )
}

/// 버킷리스트 달성 표시.
pub fn check() -> Markup {
    PreEscaped(
        r#"<path d="M5 13l4.5 4.5L19 8" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/>"#
            .to_string(),
    )
}
