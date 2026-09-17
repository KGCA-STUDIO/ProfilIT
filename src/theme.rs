//! Turns `[theme]` into CSS custom properties.
//!
//! This module is the single bridge between the schema and the stylesheet.
//! `styles.css` only ever consumes `--pf-*` tokens, so when the theme changes,
//! the stylesheet stays cached as-is and only the inline `<style>` block built
//! here changes.

use std::fmt::Write as _;

use crate::config::{Background, Card, Font, PatternName, Sheet, Theme};

/// Builds the declarations that go inside `:root { ... }`.
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
// Background
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
            // The actual image is drawn by the .pf-backdrop layer. You can't
            // apply `filter` to background-image, so blurring needs a separate element.
            let _ = writeln!(css, "  --pf-background: transparent;");
            let _ = writeln!(css, "  --pf-background-image: url(\"{}\");", src.0);

            // Repeat tiles at the image's native size, so background-size is left alone.
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

/// Draws the pattern as a CSS gradient. No image file needed, and since color
/// and size are just tokens, the editor UI reflects changes instantly.
///
/// The pattern element's thickness scales with `size` on purpose: with a fixed
/// px value, increasing `size` would just make the dots look smaller and spread out.
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
// Sheet & card
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
// Fonts
// ─────────────────────────────────────────────────────────────────────────────

fn font_variables(font: &Font, css: &mut String) {
    // Always Some once validation has passed.
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

    /// An image background must go to its own dedicated layer, not the body background.
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
        // Repeat has to keep the image's native size for it to tile properly.
        assert!(css.contains("--pf-background-fit: auto;"));
        assert!(css.contains("--pf-background-repeat: repeat;"));
        assert!(css.contains("--pf-background-blur: 8px;"));
        assert!(css.contains("--pf-background-overlay: #ffffff80;"));
    }

    /// The pattern element's thickness must scale with size so the whole pattern grows together.
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

    /// With no heading_preset set, headings should fall back to the body font.
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
