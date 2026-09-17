//! 설정 검증.
//!
//! 렌더러와 별개로 존재하는 이유는, 편집 경로가 늘어나도 검증 규칙은 하나여야
//! 하기 때문입니다. CLI 의 `check` 명령, 로컬 편집 UI 의 저장 버튼, 나중에 붙일
//! 브라우저 관리자 모드가 전부 이 함수를 재사용합니다.

use std::path::Path;

use crate::config::{
    AssetPath, Background, Config, ContactItem, ContactKind, Decoration, Font, FontPreset, Platform,
    Section, SectionBody, SCHEMA_VERSION,
};
use crate::i18n::{self, Text};
use crate::message::Message;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// 빌드를 중단시킵니다.
    Error,
    /// 빌드는 되지만 의도한 결과가 아닐 가능성이 큽니다.
    Warning,
}

/// 하나의 문제.
///
/// 문장이 아니라 **키와 인자**를 들고 있습니다. 어느 언어로 보여줄지는
/// 화면에 내보내는 쪽이 정합니다 — 검증기가 한국어 문장을 만들어 버리면
/// 편집기를 영어로 바꿔도 진단만 한국어로 남습니다.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    /// 문제가 난 위치를 TOML 경로처럼 표기합니다. 예: `sections[2].items[0].url`
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

/// `root` 는 에셋 상대 경로의 기준 디렉터리(보통 `profile.toml` 이 있는 곳)입니다.
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
// 언어
// ─────────────────────────────────────────────────────────────────────────────

/// 선언한 언어마다 문구 파일이 있는지, 번역이 얼마나 빠졌는지 봅니다.
///
/// 빠진 번역은 **언어당 한 건**으로 모아서 알립니다. 항목마다 경고를 내면
/// 언어 하나 추가했을 때 수십 줄이 쏟아져 정작 중요한 오류가 묻힙니다.
fn check_languages(config: &Config, root: &Path, out: &mut Vec<Diagnostic>) {
    let languages = config.languages();
    let default_lang = config.default_language();

    for lang in &languages {
        // 구체적인 실패 이유(파일 없음·파싱 오류)는 여기서 뭉뚱그립니다.
        // 사용자가 할 일은 어느 쪽이든 같습니다 — 그 언어 파일을 만드는 것.
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

/// 번역 가능한 값 전부를 TOML 경로와 함께 모읍니다.
///
/// 새 `Text` 필드를 스키마에 넣으면 **여기에도 추가해야 합니다.** 빠뜨리면
/// 번역 누락이 보고되지 않고 조용히 기본 언어로 나갑니다.
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
// 섹션 키 린트
//
// `Section` 은 공통 필드와 타입별 본문을 `#[serde(flatten)]` 으로 합치는데,
// serde 는 flatten 과 `deny_unknown_fields` 를 함께 쓸 수 없습니다. 그래서
// 섹션의 오타는 파싱에서 걸러지지 않고 조용히 무시됩니다. 원본 TOML 을 한 번 더
// 훑어서 그 구멍을 메웁니다.
// ─────────────────────────────────────────────────────────────────────────────

pub fn lint_section_keys(source: &str) -> Vec<Diagnostic> {
    let mut out = Vec::new();

    let Ok(table) = source.parse::<toml::Table>() else {
        // 파싱이 실패하는 경우는 호출자가 이미 보고했습니다.
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
            continue; // type 누락은 파싱 단계에서 이미 오류입니다.
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

// ─── 섹션별 검사 ─────────────────────────────────────────────────────────────

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
    // 이모지 한 글자를 의도한 필드라 길면 레이아웃이 밀립니다.
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
                // 달성 표시가 없는데 날짜가 있으면 둘 중 하나가 실수입니다.
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

/// kind 에 맞는 값 형식인지 봅니다. 렌더러가 mailto:/tel: 로 감싸기 때문에,
/// 형식이 틀리면 클릭해도 아무 일이 일어나지 않는 링크가 만들어집니다.
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
            // 전수 검사는 하지 않습니다. 오타를 잡는 것이 목적이라
            // @ 앞뒤에 내용이 있는지만 봅니다.
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
    // custom 프리셋은 스택을 직접 줘야 합니다 — 안 주면 기기 기본 글꼴로
    // 조용히 되돌아가서, 폰트를 바꿨는데 아무것도 안 바뀌는 것처럼 보입니다.
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

    // 프리셋을 골랐는데 family 도 적으면 둘 중 하나는 무시되므로 알려줍니다.
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

    // 제목에 쓰는 굵기가 실제로 그 폰트에 있는지 봅니다. 없으면 브라우저가
    // 합성 볼드로 억지로 굵게 그려서 글자 모양이 뭉개집니다.
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

// ─── 공통 검사 ───────────────────────────────────────────────────────────────

/// 허용 스킴만 통과시킵니다. `javascript:` 같은 스킴이 링크로 들어가면
/// 생성된 페이지에 그대로 실행 가능한 코드가 박히므로 여기서 막습니다.
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

/// `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa` 를 검사합니다.
/// `#` 으로 시작하지 않으면 CSS 함수나 키워드로 보고 통과시킵니다.
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

/// 숫자 + 단위 형태인지 봅니다. `0` 은 단위 없이도 유효합니다.
///
/// CSS 길이값을 전부 파싱하지는 않습니다(`calc()` 등). 목적은 `"22"` 처럼 단위를
/// 빠뜨린 값을 잡는 것입니다 — 단위 없는 값은 CSS 가 통째로 무시해서, 무늬
/// 크기나 글자 크기를 바꿨는데 화면이 그대로인 상황을 만듭니다.
fn check_css_length(path: &str, value: &str, out: &mut Vec<Diagnostic>) {
    const UNITS: [&str; 8] = ["px", "rem", "em", "%", "vw", "vh", "pt", "ch"];

    let trimmed = value.trim();
    if trimmed.is_empty() {
        out.push(Diagnostic::error(path, "msg.length.empty"));
        return;
    }
    // calc() 같은 함수 표기는 통과시킵니다.
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
