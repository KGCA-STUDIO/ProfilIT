//! `Config` 를 HTML 로 옮깁니다.
//!
//! maud 가 모든 문자열을 자동으로 이스케이프하므로, 설정 파일의 내용이 그대로
//! 태그로 해석되는 일은 없습니다. 이스케이프를 건너뛰는 곳은 `PreEscaped` 로
//! 감싼 내장 SVG 뿐이고, 그건 우리가 쓴 상수입니다.

use maud::{html, Markup, DOCTYPE};

use crate::config::{
    ChecklistItem, Config, ContactItem, ContactKind, Decoration, LinkItem, Section, SectionBody,
    Social, TimelineItem,
};
use crate::i18n::{Strings, Text};
use crate::{decor, icons, theme};

/// 한 언어를 그리는 데 필요한 것들.
///
/// 언어 코드와 UI 문구를 함수마다 끌고 다니는 대신 묶었습니다. 렌더러의 거의
/// 모든 함수가 둘 다 필요하기 때문입니다.
pub struct Ctx<'a> {
    pub config: &'a Config,
    /// 지금 그리는 언어.
    pub lang: &'a str,
    /// 번역이 빠졌을 때 돌아갈 언어.
    pub fallback: &'a str,
    pub strings: &'a Strings,
    /// 언어 선택기에 넣을 (코드, 표시 이름, 출력 루트 기준 경로).
    /// 경로는 기본 언어가 `""`, 나머지는 `"en/"` 같은 꼴입니다.
    pub languages: &'a [(String, String, String)],
    /// 이 페이지에서 출력 루트로 돌아가는 접두사. 기본 언어는 `""`,
    /// 하위 디렉터리에 놓이는 언어는 `"../"`.
    pub prefix: &'a str,
}

impl Ctx<'_> {
    /// 번역 가능한 값을 지금 언어로 풉니다.
    fn t<'t>(&self, text: &'t Text) -> &'t str {
        text.get(self.lang, self.fallback)
    }

    /// 에셋 경로를 이 페이지 기준으로 바꿉니다.
    ///
    /// 스타일시트·스크립트·이미지는 출력 루트에 한 벌만 두고 모든 언어판이
    /// 공유합니다. 언어마다 복사하면 용량이 배로 늘고, 이미지를 바꿨을 때
    /// 일부 언어판만 옛 파일을 가리키게 됩니다.
    fn asset_path(&self, path: &str) -> String {
        format!("{}{}", self.prefix, path)
    }

    /// 다른 언어판으로 가는 링크.
    fn relative_path(&self, path: &str) -> String {
        let joined = format!("{}{}", self.prefix, path);
        // 같은 디렉터리의 index 를 가리킬 때 빈 href 는 유효하지 않습니다.
        if joined.is_empty() {
            "./".to_string()
        } else {
            joined
        }
    }
}

/// 완성된 HTML 문서.
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
            // 검색엔진에 같은 내용의 다른 언어판이 있음을 알립니다.
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

            // 프리셋이 제공하는 주소 + 직접 넣은 주소. 중복은 제거됩니다.
            @for url in config.theme.font.stylesheet_urls() {
                link rel="stylesheet" href=(url);
            }
            link rel="stylesheet" href=(ctx.asset_path("styles.css"));

            // 테마 토큰. styles.css 는 이 값들을 소비만 합니다.
            style { (theme_block(config)) }
        }
    }
}

fn theme_block(config: &Config) -> String {
    format!(":root {{\n{}}}\n", theme::css_variables(&config.theme))
}

/// `base_url` 이 있으면 절대 경로로, 없으면 상대 경로 그대로 둡니다.
/// og:image 는 절대 경로가 아니면 대부분의 플랫폼이 무시합니다.
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
                // 사진이 없으면 이름 첫 글자로 자리를 채웁니다. 장식이므로
                // 낭독기에는 이름이 두 번 읽히지 않도록 숨깁니다.
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
                    // 직접 넣은 아이콘이 있으면 플랫폼과 상관없이 그것을 씁니다.
                    // 내장 글리프는 브랜드 마크를 흉내 낸 단순한 모양이라,
                    // 진짜 로고를 쓰고 싶은 사람에게 길을 막지 않습니다.
                    //
                    // 파일은 `<image>` 로 참조합니다. 내용을 읽어 인라인으로
                    // 넣으면 `currentColor` 가 먹지만, 남의 파일을 그대로
                    // 펼쳐 넣는 셈이라 스크립트가 섞여 들어올 수 있습니다.
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
// 섹션
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
                        // 이모지는 장식입니다. 낭독기가 "불꽃 요리" 처럼 읽으면
                        // 오히려 방해가 되므로 접근성 트리에서 뺍니다.
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
    // 바에만 의존하지 않도록 숫자를 글로도 적습니다.
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
                // 취소선은 __label 에만 겁니다. 상위에 걸면 날짜 배지 앞
                // 공백까지 선이 그어집니다.
                span class="pf-checklist__label" {
                    // 상태를 색과 취소선으로만 전하지 않습니다.
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
            // 공유 버튼은 <a> 의 형제입니다. 링크 안에 버튼을 넣으면 유효하지
            // 않은 HTML 이고 키보드 순회가 깨집니다.
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

/// 언어 선택기. 언어가 하나뿐이면 아무것도 그리지 않습니다.
///
/// `<select>` 가 아니라 링크 목록인 이유: 각 언어판이 별도 정적 페이지라
/// 자바스크립트 없이도 동작하고, 검색엔진이 따라갈 수 있습니다.
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

/// 브라우저에서 쓰는 문구. card.js 가 이 블록을 읽습니다 — 스크립트에 한국어를
/// 박아두면 다른 언어판에서 토스트만 한국어로 나옵니다.
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

/// vCard 생성용 데이터.
///
/// 화면용 마크업을 긁는 것보다 이 블록을 읽는 쪽이 안전합니다 — 라벨을 바꾸거나
/// 섹션을 숨겨도 연락처 저장이 계속 동작합니다.
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
// 문자열 도우미
// ─────────────────────────────────────────────────────────────────────────────

/// 줄바꿈을 `<br>` 로 바꿉니다. 각 조각은 maud 가 이스케이프합니다.
fn with_line_breaks(text: &str) -> Markup {
    html! {
        @for (i, line) in text.trim().lines().enumerate() {
            @if i > 0 { br; }
            (line)
        }
    }
}

/// 빈 줄을 기준으로 문단을 나눕니다.
fn paragraphs(text: &str) -> Vec<&str> {
    text.split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect()
}

/// 표시용으로 스킴을 뗍니다. 주소가 길면 카드 폭을 넘기기 때문입니다.
fn strip_scheme(url: &str) -> &str {
    url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
}

/// `tel:` 에는 숫자와 `+` 만 남깁니다. 공백이나 하이픈이 섞이면 일부
/// 기기에서 전화 앱이 열리지 않습니다.
fn phone_href(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == '+')
        .collect()
}

/// 아바타 자리를 채울 첫 글자.
fn first_grapheme(name: &str) -> String {
    name.trim().chars().next().map(String::from).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// 소셜 아이콘을 만듭니다. 테스트에서 `Ctx` 를 통째로 짓기는 번거로워서
    /// 렌더 결과만 보는 작은 도우미를 둡니다.
    fn social_html(icon: Option<&str>, prefix: &str) -> String {
        let strings = Strings::load("ko", Path::new(".")).unwrap_or_default();
        // 이 테스트가 보는 것은 소셜 한 줄뿐이라 최소 설정이면 충분합니다.
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

    /// 내장 글리프가 있는 플랫폼이어도 직접 넣은 아이콘이 이깁니다.
    ///
    /// 내장 아이콘은 브랜드 마크를 흉내 낸 단순한 모양이라, 진짜 로고를 쓰려는
    /// 사람의 길을 막지 않아야 합니다.
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

    /// 하위 디렉터리에 놓이는 언어판에서도 같은 파일을 가리켜야 합니다.
    ///
    /// 에셋은 출력 루트에 한 벌만 두므로 `/en/` 페이지는 `../` 를 거쳐야
    /// 합니다. 접두사를 빼먹으면 기본 언어에서만 보이고 번역판에서 깨지는데,
    /// 화면을 열어보기 전까지 모르는 종류의 고장입니다.
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

    /// 설정 파일 내용이 태그로 해석되면 안 됩니다.
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
