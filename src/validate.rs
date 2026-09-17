//! Config validation.
//!
//! This lives separately from the renderer so that no matter how many editing
//! paths we add, there's still just one set of validation rules. The CLI's
//! `check` command, the save button in the local editor UI, and whatever
//! browser-based admin mode gets bolted on later all reuse this same function.

use std::path::Path;

use crate::config::{
    AssetPath, Background, Config, ContactItem, ContactKind, Decoration, Font, FontPreset, Platform,
    Section, SectionBody, SCHEMA_VERSION,
};
use crate::i18n::{self, Text};
use crate::message::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Stops the build.
    Error,
    /// The build still succeeds, but this is probably not what was intended.
    Warning,
}

/// A single issue.
///
/// This carries a **key and arguments**, not a finished sentence. Whichever
/// side renders it decides the language — if the validator baked in Korean
/// sentences directly, switching the editor to English would still leave
/// diagnostics stuck in Korean.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    /// The location of the issue, written like a TOML path, e.g. `sections[2].items[0].url`
    pub path: String,
    pub message: Message,
}

impl Diagnostic {
    fn error(path: impl Into<String>, message: impl Into<Message>) -> Self {
        Diagnostic {
            severity: Severity::Error,
            path: path.into(),
            message: message.into(),
        }
    }

    fn warning(path: impl Into<String>, message: impl Into<Message>) -> Self {
        Diagnostic {
            severity: Severity::Warning,
            path: path.into(),
            message: message.into(),
        }
    }
}

/// `root` is the base directory that asset paths are relative to (usually wherever `profile.toml` lives).
pub fn validate(config: &Config, root: &Path) -> Vec<Diagnostic> {
    let mut out = Vec::new();

    check_schema_version(config, &mut out);
    check_site(config, root, &mut out);
    check_profile(config, root, &mut out);
    check_socials(config, root, &mut out);
    check_sections(config, root, &mut out);
    check_theme(config, root, &mut out);
    check_languages(config, root, &mut out);

    out
}

// ─────────────────────────────────────────────────────────────────────────────
// Languages
// ─────────────────────────────────────────────────────────────────────────────

/// Checks that a strings file exists for every declared language, and how much
/// translation is missing.
///
/// Missing translations are rolled up into **one diagnostic per language**.
/// Warning on every individual missing string would flood the output with
/// dozens of lines the moment someone adds a new language, burying the errors
/// that actually matter.
fn check_languages(config: &Config, root: &Path, out: &mut Vec<Diagnostic>) {
    let languages = config.languages();
    let default_lang = config.default_language();

    for lang in &languages {
        // We collapse the specific failure reason (missing file vs. parse error)
        // here — either way, what the user needs to do is the same: create that
        // language's strings file.
        if i18n::Strings::load(lang, root).is_err() {
            out.push(Diagnostic::error(
                format!("site.languages ({lang})"),
                Message::new("msg.lang.notFound")
                    .with("lang", lang)
                    .with("builtin", i18n::builtin_codes().join(", ")),
            ));
        }
    }

    let texts = collect_texts(config);

    for lang in languages.iter().filter(|l| *l != default_lang) {
        let missing: Vec<&str> = texts
            .iter()
            .filter(|(_, text)| !text.has(lang))
            .map(|(path, _)| path.as_str())
            .collect();

        if missing.is_empty() {
            continue;
        }

        const SHOWN: usize = 4;
        let sample = missing
            .iter()
            .take(SHOWN)
            .copied()
            .collect::<Vec<_>>()
            .join(", ");
        let rest = missing.len().saturating_sub(SHOWN);
        let tail = if rest > 0 {
            format!(" 외 {rest}곳")
        } else {
            String::new()
        };

        out.push(Diagnostic::warning(
            format!("site.languages ({lang})"),
            Message::new("msg.lang.missingTranslations")
                .with("count", missing.len())
                .with("lang", lang)
                .with("default", default_lang)
                .with("sample", format!("{sample}{tail}")),
        ));
    }
}

/// Collects every translatable value along with its TOML path.
///
/// If you add a new `Text` field to the schema, **you must add it here too.**
/// Miss this and missing translations won't be reported — they'll just
/// silently fall back to the default language.
fn collect_texts<'a>(config: &'a Config) -> Vec<(String, &'a Text)> {
    let mut texts: Vec<(String, &'a Text)> = Vec::new();

    let mut push = |path: String, text: &'a Text| texts.push((path, text));

    push("site.title".into(), &config.site.title);
    if let Some(t) = &config.site.description {
        push("site.description".into(), t);
    }

    push("profile.name".into(), &config.profile.name);
    if let Some(t) = &config.profile.tagline {
        push("profile.tagline".into(), t);
    }
    if let Some(t) = &config.profile.bio {
        push("profile.bio".into(), t);
    }
    if let Some(t) = &config.profile.location {
        push("profile.location".into(), t);
    }

    for (i, social) in config.socials.iter().enumerate() {
        if let Some(t) = &social.label {
            push(format!("socials[{i}].label"), t);
        }
    }

    for (i, section) in config.sections.iter().enumerate() {
        let base = format!("sections[{i}]");
        if let Some(t) = &section.title {
            push(format!("{base}.title"), t);
        }

        match &section.body {
            SectionBody::About { body } => push(format!("{base}.body"), body),

            SectionBody::Timeline { items } => {
                for (j, item) in items.iter().enumerate() {
                    let p = format!("{base}.items[{j}]");
                    push(format!("{p}.title"), &item.title);
                    if let Some(t) = &item.period {
                        push(format!("{p}.period"), t);
                    }
                    if let Some(t) = &item.subtitle {
                        push(format!("{p}.subtitle"), t);
                    }
                    if let Some(t) = &item.description {
                        push(format!("{p}.description"), t);
                    }
                }
            }

            SectionBody::Checklist { items, .. } => {
                for (j, item) in items.iter().enumerate() {
                    let p = format!("{base}.items[{j}]");
                    push(format!("{p}.text"), &item.text);
                    if let Some(t) = &item.date {
                        push(format!("{p}.date"), t);
                    }
                    if let Some(t) = &item.note {
                        push(format!("{p}.note"), t);
                    }
                }
            }

            SectionBody::Tags { items } => {
                for (j, tag) in items.iter().enumerate() {
                    push(format!("{base}.items[{j}]"), tag.text());
                }
            }

            SectionBody::Links { items } => {
                for (j, item) in items.iter().enumerate() {
                    let p = format!("{base}.items[{j}]");
                    push(format!("{p}.title"), &item.title);
                    if let Some(t) = &item.subtitle {
                        push(format!("{p}.subtitle"), t);
                    }
                    if let Some(t) = &item.badge {
                        push(format!("{p}.badge"), t);
                    }
                }
            }

            SectionBody::Contact { items } => {
                for (j, item) in items.iter().enumerate() {
                    let p = format!("{base}.items[{j}]");
                    push(format!("{p}.value"), &item.value);
                    if let Some(t) = &item.label {
                        push(format!("{p}.label"), t);
                    }
                }
            }
        }
    }

    if let Some(t) = &config.footer.text {
        push("footer.text".into(), t);
    }

    texts
}

pub fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(|d| d.severity == Severity::Error)
}

// ─────────────────────────────────────────────────────────────────────────────
// Section key linting
//
// `Section` merges common fields with a type-specific body via
// `#[serde(flatten)]`, but serde can't combine flatten with
// `deny_unknown_fields`. That means typos in section keys slip past parsing
// and get silently ignored. We patch that hole by re-scanning the raw TOML.
// ─────────────────────────────────────────────────────────────────────────────

pub fn lint_section_keys(source: &str) -> Vec<Diagnostic> {
    let mut out = Vec::new();

    let Ok(table) = source.parse::<toml::Table>() else {
        // If parsing fails, the caller has already reported it.
        return out;
    };
    let Some(sections) = table.get("sections").and_then(|v| v.as_array()) else {
        return out;
    };

    for (i, section) in sections.iter().enumerate() {
        let Some(section) = section.as_table() else {
            continue;
        };
        let Some(type_name) = section.get("type").and_then(|v| v.as_str()) else {
            continue; // A missing `type` is already caught as an error during parsing.
        };

        let body_keys = SectionBody::body_keys(type_name);
        for key in section.keys() {
            let known = SectionBody::COMMON_KEYS.contains(&key.as_str())
                || body_keys.contains(&key.as_str());
            if !known {
                out.push(Diagnostic::warning(
                    format!("sections[{i}].{key}"),
                    Message::new("msg.section.unknownKey").with("type", type_name),
                ));
            }
        }
    }

    out
}

// ─── Per-section checks ──────────────────────────────────────────────────────

fn check_schema_version(config: &Config, out: &mut Vec<Diagnostic>) {
    if config.schema_version != SCHEMA_VERSION {
        out.push(Diagnostic::error(
            "schema_version",
            Message::new("msg.schemaVersion")
                .with("found", config.schema_version)
                .with("supported", SCHEMA_VERSION),
        ));
    }
}

fn check_site(config: &Config, root: &Path, out: &mut Vec<Diagnostic>) {
    if config.site.title.is_empty() {
        out.push(Diagnostic::error("site.title", "msg.site.titleEmpty"));
    }

    match &config.site.base_url {
        None => out.push(Diagnostic::warning(
            "site.base_url",
            "msg.site.noBaseUrl",
        )),
        Some(url) => check_url("site.base_url", url, out),
    }

    check_optional_asset("site.og_image", &config.site.og_image, root, out);
    check_optional_asset("site.favicon", &config.site.favicon, root, out);
}

fn check_profile(config: &Config, root: &Path, out: &mut Vec<Diagnostic>) {
    if config.profile.name.is_empty() {
        out.push(Diagnostic::error("profile.name", "msg.profile.nameEmpty"));
    }
    check_optional_asset("profile.avatar", &config.profile.avatar, root, out);
}

fn check_socials(config: &Config, root: &Path, out: &mut Vec<Diagnostic>) {
    for (i, social) in config.socials.iter().enumerate() {
        let base = format!("socials[{i}]");
        check_url(&format!("{base}.url"), &social.url, out);

        if social.platform == Platform::Custom && social.icon.is_none() {
            out.push(Diagnostic::error(
                format!("{base}.icon"),
                "msg.social.customNeedsIcon",
            ));
        }
        check_optional_asset(&format!("{base}.icon"), &social.icon, root, out);
    }
}

fn check_sections(config: &Config, root: &Path, out: &mut Vec<Diagnostic>) {
    let default_lang = config.default_language();
    if config.visible_sections().next().is_none() {
        out.push(Diagnostic::warning(
            "sections",
            "msg.sections.none",
        ));
    }

    for (i, section) in config.sections.iter().enumerate() {
        check_section(&format!("sections[{i}]"), section, root, default_lang, out);
    }
}

fn check_section(
    base: &str,
    section: &Section,
    root: &Path,
    default_lang: &str,
    out: &mut Vec<Diagnostic>,
) {
    // This field is meant to hold a single emoji; anything longer pushes the layout out of shape.
    if let Some(icon) = &section.icon {
        if icon.chars().count() > 2 {
            out.push(Diagnostic::warning(
                format!("{base}.icon"),
                "msg.section.iconTooLong",
            ));
        }
    }

    match &section.body {
        SectionBody::About { body } => {
            if body.is_empty() {
                out.push(Diagnostic::error(
                    format!("{base}.body"),
                    "msg.about.bodyEmpty",
                ));
            }
        }

        SectionBody::Timeline { items } => {
            if items.is_empty() {
                out.push(Diagnostic::warning(
                    format!("{base}.items"),
                    "msg.timeline.noItems",
                ));
            }
            for (j, item) in items.iter().enumerate() {
                let path = format!("{base}.items[{j}]");
                if item.title.is_empty() {
                    out.push(Diagnostic::error(
                        format!("{path}.title"),
                        "msg.item.titleEmpty",
                    ));
                }
                if let Some(url) = &item.url {
                    check_url(&format!("{path}.url"), url, out);
                }
            }
        }

        SectionBody::Checklist { items, .. } => {
            if items.is_empty() {
                out.push(Diagnostic::warning(
                    format!("{base}.items"),
                    "msg.checklist.noItems",
                ));
            }
            for (j, item) in items.iter().enumerate() {
                let path = format!("{base}.items[{j}]");
                if item.text.is_empty() {
                    out.push(Diagnostic::error(
                        format!("{path}.text"),
                        "msg.checklist.textEmpty",
                    ));
                }
                // A date with no "done" flag suggests one of the two is a mistake.
                if !item.done && item.date.is_some() {
                    out.push(Diagnostic::warning(
                        format!("{path}.date"),
                        "msg.checklist.dateWithoutDone",
                    ));
                }
            }
        }

        SectionBody::Tags { items } => {
            if items.is_empty() {
                out.push(Diagnostic::warning(
                    format!("{base}.items"),
                    "msg.tags.noItems",
                ));
            }
            for (j, tag) in items.iter().enumerate() {
                if tag.text().is_empty() {
                    out.push(Diagnostic::error(
                        format!("{base}.items[{j}]"),
                        "msg.tag.empty",
                    ));
                }
                if let Some(icon) = tag.icon() {
                    if icon.chars().count() > 2 {
                        out.push(Diagnostic::warning(
                            format!("{base}.items[{j}].icon"),
                            "msg.tag.iconTooLong",
                        ));
                    }
                }
            }
        }

        SectionBody::Links { items } => {
            if items.iter().all(|i| !i.enabled) {
                out.push(Diagnostic::warning(
                    format!("{base}.items"),
                    "msg.links.noVisible",
                ));
            }
            for (j, item) in items.iter().enumerate() {
                let path = format!("{base}.items[{j}]");
                if item.title.is_empty() {
                    out.push(Diagnostic::error(
                        format!("{path}.title"),
                        "msg.link.titleEmpty",
                    ));
                }
                check_url(&format!("{path}.url"), &item.url, out);
                check_optional_asset(&format!("{path}.thumbnail"), &item.thumbnail, root, out);

                if let Some(badge) = &item.badge {
                    if badge.get(default_lang, default_lang).chars().count() > 8 {
                        out.push(Diagnostic::warning(
                            format!("{path}.badge"),
                            "msg.link.badgeTooLong",
                        ));
                    }
                }
            }
        }

        SectionBody::Contact { items } => {
            if items.is_empty() {
                out.push(Diagnostic::warning(
                    format!("{base}.items"),
                    "msg.contact.noItems",
                ));
            }
            for (j, item) in items.iter().enumerate() {
                check_contact_item(&format!("{base}.items[{j}]"), item, default_lang, out);
            }
        }
    }
}

/// Checks that the value's format matches its kind. The renderer wraps it in
/// mailto:/tel:, so a bad format produces a link that does nothing when clicked.
fn check_contact_item(
    base: &str,
    item: &ContactItem,
    default_lang: &str,
    out: &mut Vec<Diagnostic>,
) {
    let value = item.value.get(default_lang, default_lang).trim();

    if value.is_empty() {
        out.push(Diagnostic::error(
            format!("{base}.value"),
            "msg.contact.valueEmpty",
        ));
        return;
    }

    match item.kind {
        ContactKind::Email => {
            // We're not doing a full validation here. The goal is just to catch
            // typos, so we only check that there's content on both sides of the @.
            let parts: Vec<&str> = value.split('@').collect();
            let looks_like_email =
                parts.len() == 2 && !parts[0].is_empty() && parts[1].contains('.');
            if !looks_like_email {
                out.push(Diagnostic::error(
                    format!("{base}.value"),
                    Message::new("msg.contact.badEmail").with("value", value),
                ));
            }
        }
        ContactKind::Phone => {
            let digits = value.chars().filter(|c| c.is_ascii_digit()).count();
            if digits < 7 {
                out.push(Diagnostic::error(
                    format!("{base}.value"),
                    Message::new("msg.contact.badPhone").with("value", value),
                ));
            }
        }
        ContactKind::Website => check_url(&format!("{base}.value"), value, out),
        ContactKind::Address | ContactKind::Custom => {}
    }
}

fn check_theme(config: &Config, root: &Path, out: &mut Vec<Diagnostic>) {
    match &config.theme.background {
        Background::Solid { color } => check_color("theme.background.color", color, out),
        Background::Gradient { from, to, angle } => {
            check_color("theme.background.from", from, out);
            check_color("theme.background.to", to, out);
            if *angle > 360 {
                out.push(Diagnostic::error(
                    "theme.background.angle",
                    "msg.background.angleRange",
                ));
            }
        }
        Background::Pattern {
            color,
            pattern_color,
            size,
            ..
        } => {
            check_color("theme.background.color", color, out);
            check_color("theme.background.pattern_color", pattern_color, out);
            check_css_length("theme.background.size", size, out);
            if color.trim() == pattern_color.trim() {
                out.push(Diagnostic::warning(
                    "theme.background.pattern_color",
                    "msg.background.patternSameColor",
                ));
            }
        }
        Background::Image {
            src, blur, overlay, ..
        } => {
            check_asset("theme.background.src", src, root, out);
            check_css_length("theme.background.blur", blur, out);
            if let Some(overlay) = overlay {
                check_color("theme.background.overlay", overlay, out);
            }
        }
    }

    if let Some(Decoration::Custom { top, bottom }) = &config.theme.decoration {
        check_optional_asset("theme.decoration.top", top, root, out);
        check_optional_asset("theme.decoration.bottom", bottom, root, out);
        if top.is_none() && bottom.is_none() {
            out.push(Diagnostic::warning(
                "theme.decoration",
                "msg.decoration.empty",
            ));
        }
    }

    check_color("theme.sheet.background", &config.theme.sheet.background, out);
    check_color("theme.card.background", &config.theme.card.background, out);
    check_color("theme.accent", &config.theme.accent, out);

    check_color("theme.text.heading", &config.theme.text.heading, out);
    check_color("theme.text.body", &config.theme.text.body, out);
    check_color("theme.text.card_title", &config.theme.text.card_title, out);
    check_color("theme.text.muted", &config.theme.text.muted, out);

    check_font(&config.theme.font, out);
}

fn check_font(font: &Font, out: &mut Vec<Diagnostic>) {
    // The custom preset requires an explicit font stack — without one it
    // silently falls back to the device's default font, making it look like
    // changing the font had no effect at all.
    if font.preset == FontPreset::Custom && font.family.is_none() {
        out.push(Diagnostic::error(
            "theme.font.family",
            "msg.font.customNeedsFamily",
        ));
    }
    if font.heading_preset == Some(FontPreset::Custom) && font.heading_family.is_none() {
        out.push(Diagnostic::error(
            "theme.font.heading_family",
            "msg.font.customNeedsHeadingFamily",
        ));
    }

    // If both a preset and a family are set, one of them gets ignored — flag it.
    if font.preset != FontPreset::Custom && font.family.is_some() {
        out.push(Diagnostic::warning(
            "theme.font.family",
            "msg.font.familyIgnored",
        ));
    }
    if font.heading_preset.is_some()
        && font.heading_preset != Some(FontPreset::Custom)
        && font.heading_family.is_some()
    {
        out.push(Diagnostic::warning(
            "theme.font.heading_family",
            "msg.font.headingFamilyIgnored",
        ));
    }

    if let Some(family) = &font.family {
        if family.trim().is_empty() {
            out.push(Diagnostic::error(
                "theme.font.family",
                "msg.font.familyEmpty",
            ));
        }
    }

    // Checks whether the heading weight actually exists in that font. If not,
    // the browser fakes it with synthetic bold, which distorts the glyph shapes.
    let heading_font = font.heading_preset.unwrap_or(font.preset);
    let weights = heading_font.available_weights();
    if !weights.is_empty() && !weights.contains(&font.heading_weight) {
        out.push(Diagnostic::warning(
            "theme.font.heading_weight",
            Message::new("msg.font.weightUnavailable")
                .with_key("font", heading_font.label_key())
                .with("weight", font.heading_weight)
                .with(
                    "weights",
                    weights
                        .iter()
                        .map(|w| w.to_string())
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
        ));
    }

    check_css_length("theme.font.base_size", &font.base_size, out);

    if !(1.0..=2.5).contains(&font.line_height) {
        out.push(Diagnostic::error(
            "theme.font.line_height",
            Message::new("msg.font.lineHeightRange").with("value", font.line_height),
        ));
    }

    if font.letter_spacing.trim() != "normal" {
        check_css_length("theme.font.letter_spacing", &font.letter_spacing, out);
    }

    for (i, url) in font.stylesheets.iter().enumerate() {
        check_url(&format!("theme.font.stylesheets[{i}]"), url, out);
    }
}

// ─── Shared checks ────────────────────────────────────────────────────────────

/// Only lets allowed schemes through. If a scheme like `javascript:` made it
/// into a link, it would end up as executable code embedded in the generated
/// page, so we block it here.
fn check_url(path: &str, url: &str, out: &mut Vec<Diagnostic>) {
    const ALLOWED: [&str; 4] = ["https://", "http://", "mailto:", "tel:"];

    let trimmed = url.trim();
    if trimmed.is_empty() {
        out.push(Diagnostic::error(path, "msg.url.empty"));
        return;
    }

    if !ALLOWED.iter().any(|s| trimmed.starts_with(s)) {
        out.push(Diagnostic::error(
            path,
            Message::new("msg.url.notAllowed").with("value", trimmed),
        ));
    }

    if trimmed.starts_with("http://") {
        out.push(Diagnostic::warning(
            path,
            "msg.url.insecure",
        ));
    }
}

/// Checks `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa` formats.
/// Anything not starting with `#` is assumed to be a CSS function or keyword and passed through.
fn check_color(path: &str, color: &str, out: &mut Vec<Diagnostic>) {
    let trimmed = color.trim();
    if trimmed.is_empty() {
        out.push(Diagnostic::error(path, "msg.color.empty"));
        return;
    }
    let Some(hex) = trimmed.strip_prefix('#') else {
        return;
    };

    let valid_len = matches!(hex.len(), 3 | 4 | 6 | 8);
    let valid_digits = hex.chars().all(|c| c.is_ascii_hexdigit());
    if !valid_len || !valid_digits {
        out.push(Diagnostic::error(
            path,
            Message::new("msg.color.invalid").with("value", trimmed),
        ));
    }
}

/// Checks for a number-plus-unit shape. `0` is valid with no unit.
///
/// This doesn't fully parse every CSS length form (e.g. `calc()`). The goal is
/// just to catch values like `"22"` that are missing a unit — CSS ignores
/// unitless values entirely, which makes it look like changing a pattern size
/// or font size had no effect.
fn check_css_length(path: &str, value: &str, out: &mut Vec<Diagnostic>) {
    const UNITS: [&str; 8] = ["px", "rem", "em", "%", "vw", "vh", "pt", "ch"];

    let trimmed = value.trim();
    if trimmed.is_empty() {
        out.push(Diagnostic::error(path, "msg.length.empty"));
        return;
    }
    // Function notation like calc() is passed through.
    if trimmed.contains('(') {
        return;
    }
    if trimmed == "0" {
        return;
    }

    let unit = UNITS
        .iter()
        .find(|u| trimmed.ends_with(**u) && trimmed.len() > u.len());

    match unit {
        None => out.push(Diagnostic::error(
            path,
            Message::new("msg.length.noUnit").with("value", trimmed),
        )),
        Some(unit) => {
            let number = &trimmed[..trimmed.len() - unit.len()];
            if number.trim().parse::<f32>().is_err() {
                out.push(Diagnostic::error(
                    path,
                    Message::new("msg.length.notNumber").with("value", trimmed),
                ));
            }
        }
    }
}

fn check_asset(path: &str, asset: &AssetPath, root: &Path, out: &mut Vec<Diagnostic>) {
    let full = root.join(&asset.0);
    if !full.exists() {
        out.push(Diagnostic::error(
            path,
            Message::new("msg.asset.missing").with("path", &asset.0),
        ));
    }
}

fn check_optional_asset(
    path: &str,
    asset: &Option<AssetPath>,
    root: &Path,
    out: &mut Vec<Diagnostic>,
) {
    if let Some(asset) = asset {
        check_asset(path, asset, root, out);
    }
}
