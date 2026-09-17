//! Turns `Config` into HTML.
//!
//! maud auto-escapes every string, so nothing from the config file ever gets
//! interpreted as a tag. The only place that skips escaping is the built-in
//! SVG wrapped in `PreEscaped`, and that's a constant we wrote ourselves.

use maud::{html, Markup, DOCTYPE};

use crate::config::{
    ChecklistItem, Config, ContactItem, ContactKind, Decoration, LinkItem, Section, SectionBody,
    Social, TimelineItem,
};
use crate::i18n::{Strings, Text};
use crate::{decor, icons, theme};

/// Everything needed to render a single language.
///
/// Bundled together instead of passing the language code and UI strings
/// separately to every function, since nearly every renderer function needs both.
pub struct Ctx<'a> {
    pub config: &'a Config,
    /// The language being rendered right now.
    pub lang: &'a str,
    /// Language to fall back to when a translation is missing.
    pub fallback: &'a str,
    pub strings: &'a Strings,
    /// (code, display name, path relative to the output root) for the language switcher.
    /// The path is `""` for the default language and something like `"en/"` for the rest.
    pub languages: &'a [(String, String, String)],
    /// Prefix that gets this page back to the output root. `""` for the default
    /// language, `"../"` for a language placed in a subdirectory.
    pub prefix: &'a str,
}

impl Ctx<'_> {
    /// Resolves a translatable value in the current language.
    fn t<'t>(&self, text: &'t Text) -> &'t str {
        text.get(self.lang, self.fallback)
    }

    /// Rewrites an asset path relative to this page.
    ///
    /// Stylesheets, scripts, and images live in a single copy at the output
    /// root, shared by every language version. Copying them per language
    /// would double the size, and updating an image would leave some
    /// language versions pointing at the stale file.
    fn asset_path(&self, path: &str) -> String {
        format!("{}{}", self.prefix, path)
    }

    /// Link to another language version.
    fn relative_path(&self, path: &str) -> String {
        let joined = format!("{}{}", self.prefix, path);
        // An empty href isn't valid when pointing at the index of the same directory.
        if joined.is_empty() {
            "./".to_string()
        } else {
            joined
        }
    }
}

/// The finished HTML document.
pub fn page(ctx: &Ctx) -> String {
    let markup = html! {
        (DOCTYPE)
        html lang=(ctx.lang) class="no-js" {
            (head(ctx))
            (body(ctx))
        }
    };
    format!("{}\n", markup.into_string())
}

// ─────────────────────────────────────────────────────────────────────────────
// <head>
// ─────────────────────────────────────────────────────────────────────────────

fn head(ctx: &Ctx) -> Markup {
    let config = ctx.config;
    let site = &config.site;
    let canonical = site.base_url.as_deref();
    let title = ctx.t(&site.title);
    let description = site.description.as_ref().map(|d| ctx.t(d));
    let og_image = site
        .og_image
        .as_ref()
        .map(|img| absolute_url(canonical, &img.0));

    html! {
        head {
            meta charset="utf-8";
            meta name="viewport" content="width=device-width, initial-scale=1";

            title { (title) }
            @if let Some(description) = description {
                meta name="description" content=(description);
            }
            @if let Some(url) = canonical {
                link rel="canonical" href=(url);
                meta property="og:url" content=(url);
            }
            // Tells search engines that other-language versions of this content exist.
            @for (code, _, path) in ctx.languages {
                link rel="alternate" hreflang=(code)
                    href=(absolute_url(canonical, path));
            }
            @if let Some(favicon) = &site.favicon {
                link rel="icon" href=(ctx.asset_path(&favicon.0));
            }

            meta property="og:type" content="profile";
            meta property="og:title" content=(title);
            @if let Some(description) = description {
                meta property="og:description" content=(description);
            }
            @if let Some(image) = &og_image {
                meta property="og:image" content=(image);
                meta name="twitter:card" content="summary_large_image";
            } @else {
                meta name="twitter:card" content="summary";
            }

            // URLs from the preset plus any user-supplied ones. Duplicates are removed.
            @for url in config.theme.font.stylesheet_urls() {
                link rel="stylesheet" href=(url);
            }
            link rel="stylesheet" href=(ctx.asset_path("styles.css"));

            // Theme tokens. styles.css only consumes these values.
            style { (theme_block(config)) }
        }
    }
}

fn theme_block(config: &Config) -> String {
    format!(":root {{\n{}}}\n", theme::css_variables(&config.theme))
}

/// Makes the path absolute if `base_url` is set; otherwise leaves it relative.
/// Most platforms ignore og:image unless it's an absolute URL.
fn absolute_url(base: Option<&str>, path: &str) -> String {
    match base {
        None => path.to_string(),
        Some(base) => format!("{}/{}", base.trim_end_matches('/'), path.trim_start_matches('/')),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// <body>
// ─────────────────────────────────────────────────────────────────────────────

fn body(ctx: &Ctx) -> Markup {
    let config = ctx.config;
    let card_style = match config.theme.card.style {
        crate::config::CardStyle::Fill => "fill",
        crate::config::CardStyle::Outline => "outline",
        crate::config::CardStyle::Glass => "glass",
    };

    let needs_script = config.features.share_menu || config.features.vcard_download;

    html! {
        body data-card-style=(card_style) {
            div class="pf-backdrop" aria-hidden="true" {}
            (decoration(ctx))

            div class="pf-page" {
                main class="pf-sheet" {
                    (hero(ctx))
                    @for section in config.visible_sections() {
                        (self::section(section, ctx))
                    }
                }
                (language_switcher(ctx))
                (footer(ctx))
            }

            @if config.features.vcard_download {
                (vcard_data(ctx))
            }
            @if needs_script {
                div class="pf-toast" role="status" aria-live="polite" {}
                (client_strings(ctx))
                script src=(ctx.asset_path("card.js")) defer {}
            }
        }
    }
}

fn decoration(ctx: &Ctx) -> Markup {
    let Some(decoration) = &ctx.config.theme.decoration else {
        return html! {};
    };

    html! {
        div class="pf-decor" aria-hidden="true" {
            @match decoration {
                Decoration::Preset { name } => {
                    svg class="pf-decor__top" viewBox="0 0 400 190"
                        preserveAspectRatio="xMidYMin slice" { (decor::top(*name)) }
                    svg class="pf-decor__bottom" viewBox="0 0 400 150"
                        preserveAspectRatio="xMidYMax slice" { (decor::bottom(*name)) }
                }
                Decoration::Custom { top, bottom } => {
                    @if let Some(top) = top {
                        img class="pf-decor__top" src=(ctx.asset_path(&top.0)) alt="";
                    }
                    @if let Some(bottom) = bottom {
                        img class="pf-decor__bottom" src=(ctx.asset_path(&bottom.0)) alt="";
                    }
                }
            }
        }
    }
}

fn hero(ctx: &Ctx) -> Markup {
    let config = ctx.config;
    let profile = &config.profile;
    let name = ctx.t(&profile.name);

    html! {
        header class="pf-hero" {
            @match &profile.avatar {
                Some(avatar) => {
                    img class="pf-avatar" src=(ctx.asset_path(&avatar.0)) alt=""
                        width="96" height="96" decoding="async";
                }
                // With no photo, fill the space with the name's first character.
                // It's purely decorative, so it's hidden from screen readers
                // to avoid the name being read out twice.
                None => {
                    div class="pf-avatar pf-avatar--placeholder" aria-hidden="true" {
                        (first_grapheme(name))
                    }
                }
            }

            h1 class="pf-name" { (name) }

            @if let Some(tagline) = &profile.tagline {
                p class="pf-tagline" { (ctx.t(tagline)) }
            }

            @if let Some(location) = &profile.location {
                p class="pf-location" {
                    svg viewBox="0 0 24 24" aria-hidden="true" focusable="false" {
                        (icons::location())
                    }
                    (ctx.t(location))
                }
            }

            @if let Some(bio) = &profile.bio {
                p class="pf-bio" { (with_line_breaks(ctx.t(bio))) }
            }

            @if !config.socials.is_empty() {
                nav class="pf-socials" aria-label=(ctx.strings.get("aria.social_nav")) {
                    @for social in &config.socials { (self::social(social, ctx)) }
                }
            }

            @if config.features.vcard_download || config.features.share_menu {
                div class="pf-actions" {
                    @if config.features.vcard_download {
                        button class="pf-action pf-action--primary" type="button"
                            data-action="vcard" { (ctx.strings.get("action.save_contact")) }
                    }
                    @if config.features.share_menu {
                        button class="pf-action" type="button"
                            data-action="share-page" { (ctx.strings.get("action.share")) }
                    }
                }
            }
        }
    }
}

fn social(social: &Social, ctx: &Ctx) -> Markup {
    let label = match &social.label {
        Some(label) => ctx.t(label),
        None => ctx.strings.get(social.platform.locale_key()),
    };

    html! {
        a class="pf-social" href=(social.url) target="_blank" rel="noopener noreferrer me" {
            span class="pf-sr-only" { (label) }
            svg viewBox="0 0 24 24" aria-hidden="true" focusable="false" {
                @match &social.icon {
                    // A user-supplied icon always wins, regardless of platform.
                    // The built-in glyphs are simple shapes that merely evoke
                    // the brand mark, so this doesn't block anyone who wants
                    // to use the real logo.
                    //
                    // The file is referenced with `<image>`. Reading its
                    // content and inlining it would let `currentColor` apply,
                    // but that means unpacking someone else's file wholesale,
                    // which could smuggle in a script.
                    Some(icon) => {
                        image href=(ctx.asset_path(&icon.0)) width="24" height="24" {}
                    }
                    None => (icons::platform(social.platform)),
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Sections
// ─────────────────────────────────────────────────────────────────────────────

fn section(section: &Section, ctx: &Ctx) -> Markup {
    html! {
        section class="pf-section" {
            @if let Some(title) = &section.title {
                h2 class="pf-section__title" {
                    @if let Some(icon) = &section.icon {
                        span class="pf-section__icon" aria-hidden="true" { (icon) }
                    }
                    (ctx.t(title))
                }
            }
            (section_body(&section.body, ctx))
        }
    }
}

fn section_body(body: &SectionBody, ctx: &Ctx) -> Markup {
    match body {
        SectionBody::About { body } => html! {
            div class="pf-about" {
                @for paragraph in paragraphs(ctx.t(body)) { p { (with_line_breaks(paragraph)) } }
            }
        },

        SectionBody::Timeline { items } => html! {
            ol class="pf-timeline" {
                @for item in items.iter().filter(|i| i.enabled) { (timeline_item(item, ctx)) }
            }
        },

        SectionBody::Checklist {
            items,
            show_progress,
        } => {
            let done = items.iter().filter(|i| i.done).count();
            html! {
                @if *show_progress && !items.is_empty() {
                    (progress(done, items.len(), ctx))
                }
                ul class="pf-checklist" {
                    @for item in items { (checklist_item(item, ctx)) }
                }
            }
        }

        SectionBody::Tags { items } => html! {
            ul class="pf-tags" {
                @for tag in items {
                    li class="pf-tag" {
                        // Emoji here are decorative. Having a screen reader spell
                        // out "fire cooking" would just get in the way, so it's
                        // excluded from the accessibility tree.
                        @if let Some(icon) = tag.icon() {
                            span class="pf-tag__icon" aria-hidden="true" { (icon) }
                        }
                        (ctx.t(tag.text()))
                    }
                }
            }
        },

        SectionBody::Links { items } => html! {
            ul class="pf-links" {
                @for item in items.iter().filter(|i| i.enabled) {
                    (link_card(item, ctx))
                }
            }
        },

        SectionBody::Contact { items } => html! {
            ul class="pf-contact" {
                @for item in items { (contact_row(item, ctx)) }
            }
        },
    }
}

fn timeline_item(item: &TimelineItem, ctx: &Ctx) -> Markup {
    let title = ctx.t(&item.title);
    html! {
        li class="pf-timeline__item" {
            @if let Some(period) = &item.period {
                p class="pf-timeline__period" { (ctx.t(period)) }
            }
            h3 class="pf-timeline__title" {
                @match &item.url {
                    Some(url) => a href=(url) target="_blank" rel="noopener noreferrer" { (title) },
                    None => (title),
                }
            }
            @if let Some(subtitle) = &item.subtitle {
                p class="pf-timeline__subtitle" { (ctx.t(subtitle)) }
            }
            @if let Some(description) = &item.description {
                p class="pf-timeline__desc" { (with_line_breaks(ctx.t(description))) }
            }
        }
    }
}

fn progress(done: usize, total: usize, ctx: &Ctx) -> Markup {
    // Spell out the numbers in text too, so the bar isn't the only thing conveying them.
    let percent = if total == 0 { 0 } else { done * 100 / total };
    let label = ctx.strings.format(
        "checklist.progress",
        &[("total", &total.to_string()), ("done", &done.to_string())],
    );

    html! {
        p class="pf-progress" {
            span class="pf-progress__label" { (label) }
            span class="pf-progress__track" aria-hidden="true" {
                span class="pf-progress__bar"
                    style=(format!("--pf-progress-value: {percent}%")) {}
            }
        }
    }
}

fn checklist_item(item: &ChecklistItem, ctx: &Ctx) -> Markup {
    let class = if item.done {
        "pf-checklist__item pf-checklist__item--done"
    } else {
        "pf-checklist__item"
    };

    html! {
        li class=(class) {
            span class="pf-checklist__mark" aria-hidden="true" {
                @if item.done {
                    svg viewBox="0 0 24 24" focusable="false" { (icons::check()) }
                }
            }
            span class="pf-checklist__text" {
                // The strikethrough is scoped to __label only. Putting it on the
                // parent would also strike through the space before the date badge.
                span class="pf-checklist__label" {
                    // Don't convey the state through color and strikethrough alone.
                    @if item.done {
                        span class="pf-sr-only" { (ctx.strings.get("checklist.done_prefix")) }
                    }
                    (ctx.t(&item.text))
                }
                @if let Some(date) = &item.date {
                    span class="pf-checklist__date" { (ctx.t(date)) }
                }
            }
            @if let Some(note) = &item.note {
                span class="pf-checklist__note" { (ctx.t(note)) }
            }
        }
    }
}

fn link_card(item: &LinkItem, ctx: &Ctx) -> Markup {
    let title = ctx.t(&item.title);
    let class = if item.highlight {
        "pf-card pf-card--highlight"
    } else {
        "pf-card"
    };

    html! {
        li class=(class) {
            @if let Some(thumbnail) = &item.thumbnail {
                img class="pf-card__thumb" src=(ctx.asset_path(&thumbnail.0)) alt=""
                    width="44" height="44" loading="lazy" decoding="async";
            }
            // The share button is a sibling of <a>, not nested inside it — putting
            // a button inside a link is invalid HTML and breaks keyboard navigation.
            a class="pf-card__link" href=(item.url) target="_blank" rel="noopener noreferrer" {
                (title)
                @if let Some(badge) = &item.badge {
                    span class="pf-card__badge" { (ctx.t(badge)) }
                }
                @if let Some(subtitle) = &item.subtitle {
                    span class="pf-card__subtitle" { (ctx.t(subtitle)) }
                }
            }
            @if ctx.config.features.share_menu {
                button class="pf-card__share" type="button"
                    aria-label=(ctx.strings.format("aria.share_item", &[("title", title)])) {
                    svg viewBox="0 0 24 24" aria-hidden="true" focusable="false" {
                        (icons::share())
                    }
                }
            }
        }
    }
}

fn contact_row(item: &ContactItem, ctx: &Ctx) -> Markup {
    let label = match &item.label {
        Some(label) => ctx.t(label),
        None => ctx.strings.get(item.kind.locale_key()),
    };
    let value = ctx.t(&item.value);

    html! {
        li class="pf-contact__row" {
            span class="pf-contact__label" { (label) }
            @match item.kind {
                ContactKind::Email => {
                    a class="pf-contact__value" href=(format!("mailto:{value}")) { (value) }
                }
                ContactKind::Phone => {
                    a class="pf-contact__value" href=(format!("tel:{}", phone_href(value))) {
                        (value)
                    }
                }
                ContactKind::Website => {
                    a class="pf-contact__value" href=(value)
                        target="_blank" rel="noopener noreferrer" {
                        (strip_scheme(value))
                    }
                }
                ContactKind::Address | ContactKind::Custom => {
                    span class="pf-contact__value" { (value) }
                }
            }
        }
    }
}

/// Language switcher. Renders nothing when there's only one language.
///
/// Why a list of links instead of `<select>`: each language version is a
/// separate static page, so this works without JavaScript and search
/// engines can crawl it.
fn language_switcher(ctx: &Ctx) -> Markup {
    if ctx.languages.len() < 2 {
        return html! {};
    }

    html! {
        nav class="pf-languages" aria-label=(ctx.strings.get("aria.language_nav")) {
            @for (code, name, path) in ctx.languages {
                @if code == ctx.lang {
                    span class="pf-language pf-language--current" aria-current="true" { (name) }
                } @else {
                    a class="pf-language" href=(ctx.relative_path(path)) hreflang=(code) { (name) }
                }
            }
        }
    }
}

/// Strings used in the browser. card.js reads this block — hardcoding Korean
/// into the script would mean the toast shows up in Korean on every language version.
fn client_strings(ctx: &Ctx) -> Markup {
    let json = serde_json::to_string(&ctx.strings.client_subset()).unwrap_or_else(|_| "{}".into());
    html! {
        script type="application/json" id="pf-strings" { (maud::PreEscaped(json)) }
    }
}

fn footer(ctx: &Ctx) -> Markup {
    let footer = &ctx.config.footer;
    if footer.text.is_none() && !footer.show_powered_by {
        return html! {};
    }

    html! {
        footer class="pf-footer" {
            @if let Some(text) = &footer.text {
                p class="pf-footer__powered" { (ctx.t(text)) }
            }
            @if footer.show_powered_by {
                p class="pf-footer__powered" {
                    (ctx.strings.get("footer.powered_by")) " "
                    a href="https://github.com/example/profileit" rel="noopener" { "ProfileIT" }
                }
            }
        }
    }
}

/// Data for generating the vCard.
///
/// Reading this block is safer than scraping the display markup — contact
/// saving keeps working even if labels change or a section gets hidden.
fn vcard_data(ctx: &Ctx) -> Markup {
    let config = ctx.config;
    let mut email = None;
    let mut phone = None;
    let mut url = config.site.base_url.clone();

    for section in config.visible_sections() {
        let SectionBody::Contact { items } = &section.body else {
            continue;
        };
        for item in items {
            let value = ctx.t(&item.value).to_string();
            match item.kind {
                ContactKind::Email if email.is_none() => email = Some(value),
                ContactKind::Phone if phone.is_none() => phone = Some(value),
                ContactKind::Website if url.is_none() => url = Some(value),
                _ => {}
            }
        }
    }

    let data = serde_json::json!({
        "name": ctx.t(&config.profile.name),
        "tagline": config.profile.tagline.as_ref().map(|t| ctx.t(t)),
        "location": config.profile.location.as_ref().map(|t| ctx.t(t)),
        "email": email,
        "phone": phone,
        "url": url,
    });

    html! {
        script type="application/json" id="pf-vcard-data" {
            (maud::PreEscaped(data.to_string()))
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// String helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Turns line breaks into `<br>`. Each piece is escaped by maud.
fn with_line_breaks(text: &str) -> Markup {
    html! {
        @for (i, line) in text.trim().lines().enumerate() {
            @if i > 0 { br; }
            (line)
        }
    }
}

/// Splits into paragraphs on blank lines.
fn paragraphs(text: &str) -> Vec<&str> {
    text.split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect()
}

/// Strips the scheme for display, since a long URL would overflow the card width.
fn strip_scheme(url: &str) -> &str {
    url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
}

/// Keeps only digits and `+` for `tel:`. Spaces or hyphens mixed in keep the
/// phone app from opening on some devices.
fn phone_href(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '+')
        .collect()
}

/// First character used to fill the avatar placeholder.
fn first_grapheme(name: &str) -> String {
    name.trim().chars().next().map(String::from).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// Builds a social icon. Constructing a whole `Ctx` in a test is tedious,
    /// so this small helper just gives us the render output.
    fn social_html(icon: Option<&str>, prefix: &str) -> String {
        let strings = Strings::load("ko", Path::new(".")).unwrap_or_default();
        // This test only looks at a single social row, so a minimal config is enough.
        let config: Config = toml::from_str(
            "schema_version = 1
[site]
lang = \"ko\"
title = \"t\"
[profile]
name = \"n\"
",
        )
        .expect("최소 설정 파싱");
        let ctx = Ctx {
            config: &config,
            lang: "ko",
            fallback: "ko",
            strings: &strings,
            languages: &[],
            prefix,
        };
        let item = Social {
            platform: crate::config::Platform::Github,
            url: "https://github.com/example".to_string(),
            label: None,
            icon: icon.map(|p| crate::config::AssetPath(p.to_string())),
        };
        social(&item, &ctx).into_string()
    }

    /// A supplied icon wins even for a platform with a built-in glyph.
    ///
    /// The built-in icons are simple shapes that merely evoke the brand mark,
    /// so they shouldn't stand in the way of someone who wants the real logo.
    #[test]
    fn supplied_icon_beats_the_builtin_glyph() {
        let html = social_html(Some("assets/github.svg"), "");
        assert!(html.contains("assets/github.svg"), "{html}");
        assert!(!html.contains("<path"), "내장 글리프가 같이 나왔습니다: {html}");
    }

    #[test]
    fn no_icon_falls_back_to_the_builtin_glyph() {
        let html = social_html(None, "");
        assert!(!html.contains("<image"), "{html}");
        assert!(html.contains("currentColor"), "{html}");
    }

    /// A language version placed in a subdirectory must still point at the same file.
    ///
    /// Assets live in a single copy at the output root, so an `/en/` page has
    /// to go through `../`. Dropping the prefix would only show up on the
    /// default language and break on translations — the kind of failure that
    /// stays invisible until the page is actually opened.
    #[test]
    fn supplied_icon_follows_the_language_prefix() {
        let html = social_html(Some("assets/github.svg"), "../");
        assert!(html.contains("../assets/github.svg"), "{html}");
    }

    #[test]
    fn line_breaks_become_br() {
        let html = with_line_breaks("첫 줄\n둘째 줄").into_string();
        assert_eq!(html, "첫 줄<br>둘째 줄");
    }

    #[test]
    fn blank_line_splits_paragraphs() {
        assert_eq!(paragraphs("가\n\n나\n\n\n다"), vec!["가", "나", "다"]);
    }

    /// Config file content must never be interpreted as a tag.
    #[test]
    fn user_text_is_escaped() {
        let html = with_line_breaks("<script>alert(1)</script>").into_string();
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn phone_href_keeps_digits_only() {
        assert_eq!(phone_href("+82 10-1234-5678"), "+821012345678");
    }

    #[test]
    fn scheme_is_stripped_for_display() {
        assert_eq!(strip_scheme("https://example.com/"), "example.com");
    }

    #[test]
    fn og_image_becomes_absolute() {
        assert_eq!(
            absolute_url(Some("https://example.com/"), "assets/og.png"),
            "https://example.com/assets/og.png"
        );
        assert_eq!(absolute_url(None, "assets/og.png"), "assets/og.png");
    }

    #[test]
    fn avatar_placeholder_uses_first_character() {
        assert_eq!(first_grapheme("김도윤"), "김");
        assert_eq!(first_grapheme(""), "");
    }
}
