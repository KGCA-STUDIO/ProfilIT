//! `[theme]` 를 CSS 커스텀 프로퍼티로 옮깁니다.
//!
//! 이 모듈이 스키마와 스타일시트를 잇는 유일한 지점입니다. `styles.css` 는
//! `--pf-*` 토큰을 소비만 하므로, 테마가 바뀌어도 스타일시트는 그대로 캐시에
//! 남고 여기서 만든 인라인 `<style>` 블록만 달라집니다.

use std::fmt::Write as _;

use crate::config::{Background, Card, Font, PatternName, Sheet, Theme};

/// `:root { ... }` 안에 들어갈 선언들을 만듭니다.
pub fn css_variables(theme: &Theme) -> String {
    let mut css = String::new();

    background_variables(&theme.background, &mut css);
    sheet_variables(&theme.sheet, &mut css);
    card_variables(&theme.card, &mut css);

    let _ = writeln!(css, "  --pf-accent: {};", theme.accent);

    let _ = writeln!(css, "  --pf-text-heading: {};", theme.text.heading);
    let _ = writeln!(css, "  --pf-text-body: {};", theme.text.body);
    let _ = writeln!(css, "  --pf-text-card-title: {};", theme.text.card_title);
    let _ = writeln!(css, "  --pf-text-muted: {};", theme.text.muted);

    font_variables(&theme.font, &mut css);

    css
}

// ─────────────────────────────────────────────────────────────────────────────
// 배경
// ─────────────────────────────────────────────────────────────────────────────

fn background_variables(background: &Background, css: &mut String) {
    match background {
        Background::Solid { color } => {
            let _ = writeln!(css, "  --pf-background: {color};");
        }

        Background::Gradient { from, to, angle } => {
            let _ = writeln!(
                css,
                "  --pf-background: linear-gradient({angle}deg, {from} 0%, {to} 100%);"
            );
        }

        Background::Pattern {
            name,
            color,
            pattern_color,
            size,
        } => {
            let _ = writeln!(
                css,
                "  --pf-background: {};",
                pattern_value(*name, color, pattern_color, size)
            );
        }

        Background::Image {
            src,
            fit,
            position,
            blur,
            overlay,
        } => {
            // 실제 그림은 .pf-backdrop 레이어가 그립니다. background-image 에는
            // filter 를 걸 수 없어서 흐림 처리를 하려면 별도 요소가 필요합니다.
            let _ = writeln!(css, "  --pf-background: transparent;");
            let _ = writeln!(css, "  --pf-background-image: url(\"{}\");", src.0);

            // repeat 은 원본 크기로 타일링하므로 background-size 를 건드리지 않습니다.
            let (size, repeat) = match fit {
                crate::config::ImageFit::Cover => ("cover", "no-repeat"),
                crate::config::ImageFit::Contain => ("contain", "no-repeat"),
                crate::config::ImageFit::Repeat => ("auto", "repeat"),
            };
            let _ = writeln!(css, "  --pf-background-fit: {size};");
            let _ = writeln!(css, "  --pf-background-repeat: {repeat};");
            let _ = writeln!(css, "  --pf-background-position: {position};");
            let _ = writeln!(css, "  --pf-background-blur: {blur};");

            if let Some(overlay) = overlay {
                let _ = writeln!(css, "  --pf-background-overlay: {overlay};");
            }
        }
    }
}

/// 무늬를 CSS 그라디언트로 그립니다. 이미지 파일이 필요 없고, 색과 크기가
/// 토큰이라 편집 UI 에서 즉시 반영됩니다.
///
/// 무늬 요소의 두께를 `size` 에 비례시킨 이유: 고정 px 로 두면 `size` 를 키웠을 때
/// 점만 작아 보이고 간격만 벌어집니다.
fn pattern_value(name: PatternName, color: &str, ink: &str, size: &str) -> String {
    match name {
        PatternName::Dots => format!(
            "radial-gradient(circle, {ink} calc({size} / 9), transparent calc({size} / 9)) \
             0 0 / {size} {size}, {color}"
        ),
        PatternName::Grid => format!(
            "linear-gradient({ink} 1px, transparent 1px) 0 0 / {size} {size}, \
             linear-gradient(90deg, {ink} 1px, transparent 1px) 0 0 / {size} {size}, {color}"
        ),
        PatternName::Stripes => format!(
            "repeating-linear-gradient(45deg, {ink} 0 calc({size} / 2), \
             transparent calc({size} / 2) {size}), {color}"
        ),
        PatternName::Checks => format!(
            "conic-gradient({ink} 0 25%, transparent 0 50%, {ink} 0 75%, transparent 0) \
             0 0 / {size} {size}, {color}"
        ),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 명함 본체 · 카드
// ─────────────────────────────────────────────────────────────────────────────

fn sheet_variables(sheet: &Sheet, css: &mut String) {
    let _ = writeln!(css, "  --pf-sheet-bg: {};", sheet.background);
    let _ = writeln!(css, "  --pf-sheet-radius: {};", sheet.radius);
    let shadow = if sheet.shadow {
        "0 8px 30px rgb(16 62 82 / 14%)"
    } else {
        "none"
    };
    let _ = writeln!(css, "  --pf-sheet-shadow: {shadow};");
}

fn card_variables(card: &Card, css: &mut String) {
    let _ = writeln!(css, "  --pf-card-bg: {};", card.background);
    let _ = writeln!(css, "  --pf-card-radius: {};", card.radius);
    let shadow = if card.shadow {
        "0 2px 10px rgb(16 62 82 / 10%)"
    } else {
        "none"
    };
    let _ = writeln!(css, "  --pf-card-shadow: {shadow};");
}

// ─────────────────────────────────────────────────────────────────────────────
// 글씨
// ─────────────────────────────────────────────────────────────────────────────

fn font_variables(font: &Font, css: &mut String) {
    // 검증을 통과했다면 항상 Some 입니다.
    if let Some(family) = font.body_family() {
        let _ = writeln!(css, "  --pf-font-family: {family};");
    }
    if let Some(heading) = font.heading_family() {
        let _ = writeln!(css, "  --pf-font-heading-family: {heading};");
    }
    let _ = writeln!(css, "  --pf-font-heading-weight: {};", font.heading_weight);
    let _ = writeln!(css, "  --pf-font-size-base: {};", font.base_size);
    let _ = writeln!(css, "  --pf-line-height: {};", font.line_height);
    let _ = writeln!(css, "  --pf-letter-spacing: {};", font.letter_spacing);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ImageFit, TextColors};

    fn theme_with(background: Background) -> Theme {
        Theme {
            background,
            ..Theme::default()
        }
    }

    #[test]
    fn gradient_becomes_linear_gradient() {
        let css = css_variables(&theme_with(Background::Gradient {
            from: "#fff".into(),
            to: "#000".into(),
            angle: 165,
        }));
        assert!(css.contains("--pf-background: linear-gradient(165deg, #fff 0%, #000 100%);"));
    }

    /// 이미지 배경은 body 배경이 아니라 전용 레이어로 나가야 합니다.
    #[test]
    fn image_goes_to_backdrop_layer() {
        let css = css_variables(&theme_with(Background::Image {
            src: crate::config::AssetPath("assets/bg.jpg".into()),
            fit: ImageFit::Repeat,
            position: "top".into(),
            blur: "8px".into(),
            overlay: Some("#ffffff80".into()),
        }));
        assert!(css.contains("--pf-background: transparent;"));
        assert!(css.contains("--pf-background-image: url(\"assets/bg.jpg\");"));
        // repeat 은 원본 크기를 유지해야 타일이 됩니다.
        assert!(css.contains("--pf-background-fit: auto;"));
        assert!(css.contains("--pf-background-repeat: repeat;"));
        assert!(css.contains("--pf-background-blur: 8px;"));
        assert!(css.contains("--pf-background-overlay: #ffffff80;"));
    }

    /// 무늬 요소의 두께가 size 에 비례해야 크기를 키웠을 때 무늬가 함께 커집니다.
    #[test]
    fn pattern_scales_with_size() {
        let value = pattern_value(PatternName::Dots, "#fff", "#000", "40px");
        assert!(value.contains("calc(40px / 9)"));
        assert!(value.ends_with("#fff"));
    }

    #[test]
    fn shadow_toggles_to_none() {
        let theme = Theme {
            sheet: Sheet {
                shadow: false,
                ..Sheet::default()
            },
            ..Theme::default()
        };
        assert!(css_variables(&theme).contains("--pf-sheet-shadow: none;"));
    }

    /// heading_preset 이 없으면 제목도 본문 폰트를 써야 합니다.
    #[test]
    fn heading_font_falls_back_to_body_font() {
        let theme = Theme {
            font: Font {
                preset: crate::config::FontPreset::Pretendard,
                heading_preset: None,
                ..Font::default()
            },
            text: TextColors::default(),
            ..Theme::default()
        };
        let css = css_variables(&theme);
        let body = css
            .lines()
            .find(|l| l.contains("--pf-font-family:"))
            .unwrap()
            .replace("--pf-font-family:", "");
        let heading = css
            .lines()
            .find(|l| l.contains("--pf-font-heading-family:"))
            .unwrap()
            .replace("--pf-font-heading-family:", "");
        assert_eq!(body.trim(), heading.trim());
    }
}
