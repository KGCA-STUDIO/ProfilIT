//! Schema definition for `profile.toml`.
//!
//! This module **does not depend on any particular editing method.** The local
//! editing UI, the static renderer, and a browser-based admin mode to be added
//! later all share the same types. When adding a new field, attach
//! `#[serde(default)]` so existing files keep loading, and only bump
//! `SCHEMA_VERSION` and add a migration for breaking changes.

use serde::{Deserialize, Serialize};

use crate::i18n::Text;

/// Schema version this binary can read.
pub const SCHEMA_VERSION: u32 = 1;

/// Asset path. Relative to the directory containing `profile.toml`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct AssetPath(pub String);

fn default_true() -> bool {
    true
}

// ─────────────────────────────────────────────────────────────────────────────
// Root
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema_version: u32,
    pub site: Site,
    pub profile: Profile,
    #[serde(default)]
    pub socials: Vec<Social>,
    /// Array order is display order.
    #[serde(default)]
    pub sections: Vec<Section>,
    #[serde(default)]
    pub theme: Theme,
    #[serde(default)]
    pub features: Features,
    #[serde(default)]
    pub footer: Footer,
    #[serde(default)]
    pub deploy: Deploy,
}

impl Config {
    /// Returns, in order, only the sections that actually render on screen.
    pub fn visible_sections(&self) -> impl Iterator<Item = &Section> {
        self.sections.iter().filter(|s| s.enabled)
    }

    /// Languages to build. **The default language always comes first** and
    /// duplicates are removed.
    ///
    /// The first language ends up at the site root (`/`), so order here is
    /// layout. People often forget to list the default language in
    /// `languages`, so we patch that up here.
    pub fn languages(&self) -> Vec<String> {
        let mut langs = vec![self.site.lang.clone()];
        for lang in &self.site.languages {
            if !langs.contains(lang) {
                langs.push(lang.clone());
            }
        }
        langs
    }

    /// Default language. Where we fall back to when a translation is missing.
    pub fn default_language(&self) -> &str {
        &self.site.lang
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Site metadata
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Site {
    /// `<title>` and og:title.
    pub title: Text,
    #[serde(default)]
    pub description: Option<Text>,
    /// Deployed URL. Needed to make og:url / canonical absolute.
    #[serde(default)]
    pub base_url: Option<String>,
    /// Default language. This is the one that lands at the site root (`/`).
    #[serde(default = "default_lang")]
    pub lang: String,
    /// Languages to build. Leave empty to build only the default language.
    /// If the default language is missing here, the renderer adds it at the front.
    #[serde(default)]
    pub languages: Vec<String>,
    #[serde(default)]
    pub og_image: Option<AssetPath>,
    #[serde(default)]
    pub favicon: Option<AssetPath>,
}

fn default_lang() -> String {
    "ko".to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// Profile (top of the card)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub name: Text,
    /// One line under the name. A title or short intro.
    #[serde(default)]
    pub tagline: Option<Text>,
    /// Multiple lines allowed. The renderer turns line breaks into `<br>`.
    #[serde(default)]
    pub bio: Option<Text>,
    #[serde(default)]
    pub avatar: Option<AssetPath>,
    /// Short location, e.g. "Seoul".
    #[serde(default)]
    pub location: Option<Text>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Socials
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Social {
    pub platform: Platform,
    pub url: String,
    /// Screen-reader label. Falls back to the platform name if omitted.
    #[serde(default)]
    pub label: Option<Text>,
    /// Path to a custom icon (SVG).
    ///
    /// Works for any platform — the built-in glyphs are simple shapes that
    /// merely gesture at the brand mark, so if you want the real logo, point
    /// this at a file and it wins. Required when `platform` is `custom`,
    /// since there's no built-in glyph for that.
    #[serde(default)]
    pub icon: Option<AssetPath>,
}

/// Platforms that ship with a built-in icon.
///
/// To add an icon, add a variant here and register the SVG in `icons.rs`.
/// This is deliberately left open as the easiest place for a contributor to
/// send a PR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    Instagram,
    Youtube,
    Threads,
    Tiktok,
    X,
    Facebook,
    Naver,
    NaverBlog,
    KakaoTalk,
    Github,
    Linkedin,
    Email,
    Rss,
    /// Not in the built-in list. Must be paired with an `icon` field.
    Custom,
}

impl Platform {
    /// Key into the locale files. The display name comes from
    /// `locales/<code>.json` — it needs to vary per language, e.g.
    /// "네이버" vs "Naver".
    pub fn locale_key(self) -> &'static str {
        match self {
            Platform::Instagram => "platform.instagram",
            Platform::Youtube => "platform.youtube",
            Platform::Threads => "platform.threads",
            Platform::Tiktok => "platform.tiktok",
            Platform::X => "platform.x",
            Platform::Facebook => "platform.facebook",
            Platform::Naver => "platform.naver",
            Platform::NaverBlog => "platform.naver_blog",
            Platform::KakaoTalk => "platform.kakao_talk",
            Platform::Github => "platform.github",
            Platform::Linkedin => "platform.linkedin",
            Platform::Email => "platform.email",
            Platform::Rss => "platform.rss",
            Platform::Custom => "platform.custom",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Sections
//
// The fields shared by every section (title / icon / enabled) live at the top
// level, and each `type`'s own body is flattened in. serde won't let you
// combine `flatten` with `deny_unknown_fields`, so typos in a section body
// can't be caught at parse time. Instead, `validate::lint_section_keys`
// re-scans the raw TOML and reports unknown keys as warnings.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Section {
    /// Section title. If omitted, only the body renders, with no heading.
    #[serde(default)]
    pub title: Option<Text>,
    /// Single emoji shown before the title.
    #[serde(default)]
    pub icon: Option<String>,
    /// If false, the section stays in the file but is dropped from the page.
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(flatten)]
    pub body: SectionBody,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SectionBody {
    /// Free-form paragraph. Bio, what you do.
    About { body: Text },
    /// Chronological entries, like education or work history.
    Timeline { items: Vec<TimelineItem> },
    /// Bucket list. Tracks whether each item is done.
    Checklist {
        items: Vec<ChecklistItem>,
        /// Show a progress bar at the top.
        #[serde(default = "default_true")]
        show_progress: bool,
    },
    /// Short keyword group, e.g. interests or skills.
    Tags { items: Vec<Tag> },
    /// List of link cards.
    Links { items: Vec<LinkItem> },
    /// Contact methods like email or phone.
    Contact { items: Vec<ContactItem> },
}

impl SectionBody {
    /// The body keys a given type uses. This is what the key linter checks against.
    pub fn body_keys(type_name: &str) -> &'static [&'static str] {
        match type_name {
            "about" => &["body"],
            "timeline" => &["items"],
            "checklist" => &["items", "show_progress"],
            "tags" => &["items"],
            "links" => &["items"],
            "contact" => &["items"],
            _ => &[],
        }
    }

    /// Keys shared by every section type.
    pub const COMMON_KEYS: &'static [&'static str] = &["type", "title", "icon", "enabled"];
}

/// An interest tag.
///
/// If you don't need an emoji, a plain string (or translation table) works fine.
///
/// ```toml
/// items = [
///   "Rust",
///   { ko = "요리", en = "Cooking" },
///   { icon = "🍳", text = { ko = "요리", en = "Cooking" } },
/// ]
/// ```
///
/// `WithIcon` must come **first**. Swap the order and `{ icon = ..., text = ... }`
/// gets misread as a translation table with language codes `icon` and `text`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Tag {
    WithIcon { icon: String, text: Text },
    Simple(Text),
}

impl Tag {
    pub fn icon(&self) -> Option<&str> {
        match self {
            Tag::WithIcon { icon, .. } => Some(icon),
            Tag::Simple(_) => None,
        }
    }

    pub fn text(&self) -> &Text {
        match self {
            Tag::WithIcon { text, .. } => text,
            Tag::Simple(text) => text,
        }
    }
}

/// An education/work history entry.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TimelineItem {
    pub title: Text,
    /// Free-form, e.g. "2018 – 2022". Not used for sorting, so no format is
    /// enforced — array order is display order.
    #[serde(default)]
    pub period: Option<Text>,
    /// Subtitle, e.g. affiliation or degree.
    #[serde(default)]
    pub subtitle: Option<Text>,
    #[serde(default)]
    pub description: Option<Text>,
    /// If set, the title becomes a link.
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// A bucket-list item.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChecklistItem {
    pub text: Text,
    #[serde(default)]
    pub done: bool,
    /// Date achieved. Free-form, e.g. "2025-04".
    #[serde(default)]
    pub date: Option<Text>,
    /// A one-line note.
    #[serde(default)]
    pub note: Option<Text>,
}

/// A link card.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LinkItem {
    pub title: Text,
    pub url: String,
    #[serde(default)]
    pub subtitle: Option<Text>,
    #[serde(default)]
    pub thumbnail: Option<AssetPath>,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Subtle wiggle animation. Respects `prefers-reduced-motion`.
    #[serde(default)]
    pub highlight: bool,
    /// Short badge text, e.g. "NEW" or an announcement.
    #[serde(default)]
    pub badge: Option<Text>,
}

/// A contact method.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContactItem {
    pub kind: ContactKind,
    /// Email address, phone number, address string, etc. The renderer wraps
    /// it in the right kind of link for `kind` (mailto:, tel:).
    pub value: Text,
    /// Label shown on screen. Falls back to the default name for `kind` if omitted.
    #[serde(default)]
    pub label: Option<Text>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContactKind {
    Email,
    Phone,
    Address,
    Website,
    /// Shown as plain text, with no link.
    Custom,
}

impl ContactKind {
    pub fn locale_key(self) -> &'static str {
        match self {
            ContactKind::Email => "contact.email",
            ContactKind::Phone => "contact.phone",
            ContactKind::Address => "contact.address",
            ContactKind::Website => "contact.website",
            ContactKind::Custom => "contact.custom",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Theme — the fields below map 1:1 to CSS custom properties.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Theme {
    pub background: Background,
    pub decoration: Option<Decoration>,
    /// The card body itself (the white sheet).
    pub sheet: Sheet,
    /// Link cards that sit inside a section.
    pub card: Card,
    pub text: TextColors,
    pub font: Font,
    /// Accent color used for the timeline dots, progress bar, and tag chips. `--pf-accent`.
    pub accent: String,
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            background: Background::default(),
            decoration: None,
            sheet: Sheet::default(),
            card: Card::default(),
            text: TextColors::default(),
            font: Font::default(),
            accent: "#2f9fd0".to_string(),
        }
    }
}

/// Background.
///
/// `solid`, `gradient`, and `pattern` all resolve to a single `--pf-background`
/// token (the CSS `background` shorthand accepts multiple layers, so a
/// pattern can be expressed here too). `image` needs blur, so it gets its own
/// layer (`.pf-backdrop`) instead — `filter` can't be applied to `background-image`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Background {
    Solid {
        color: String,
    },
    Gradient {
        from: String,
        to: String,
        #[serde(default = "default_angle")]
        angle: u16,
    },
    /// A repeating pattern drawn with a CSS gradient. No image file needed.
    Pattern {
        name: PatternName,
        /// Base color behind the pattern.
        #[serde(default = "default_pattern_base")]
        color: String,
        /// Color of the pattern itself.
        #[serde(default = "default_pattern_ink")]
        pattern_color: String,
        /// Size of one pattern tile. A CSS length value.
        #[serde(default = "default_pattern_size")]
        size: String,
    },
    Image {
        src: AssetPath,
        #[serde(default)]
        fit: ImageFit,
        /// A CSS `background-position` value.
        #[serde(default = "default_image_position")]
        position: String,
        /// Blur radius. A CSS length value. `"0px"` means no blur.
        #[serde(default = "default_image_blur")]
        blur: String,
        /// Semi-transparent overlay drawn on top of the image for readability.
        #[serde(default)]
        overlay: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PatternName {
    /// Polka dots.
    Dots,
    /// Grid.
    Grid,
    /// Diagonal stripes.
    Stripes,
    /// Checkerboard.
    Checks,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageFit {
    /// Fills the frame, cropping anything that overflows.
    #[default]
    Cover,
    /// Scales so the whole image stays visible.
    Contain,
    /// Repeats at original size, like a tile.
    Repeat,
}

fn default_angle() -> u16 {
    180
}

fn default_pattern_base() -> String {
    "#ffffff".to_string()
}

fn default_pattern_ink() -> String {
    "#e6f4fb".to_string()
}

fn default_pattern_size() -> String {
    "22px".to_string()
}

fn default_image_position() -> String {
    "center".to_string()
}

fn default_image_blur() -> String {
    "0px".to_string()
}

impl Default for Background {
    fn default() -> Self {
        Background::Solid {
            color: "#ffffff".to_string(),
        }
    }
}

/// Confetti/sticker decoration layer. Purely decorative, so it's marked `aria-hidden`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Decoration {
    /// Built-in SVG set.
    Preset { name: DecorationPreset },
    /// Custom images placed at the top and/or bottom.
    Custom {
        #[serde(default)]
        top: Option<AssetPath>,
        #[serde(default)]
        bottom: Option<AssetPath>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DecorationPreset {
    Confetti,
    Sparkle,
    Bubble,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Sheet {
    pub background: String,
    /// A CSS length value.
    pub radius: String,
    pub shadow: bool,
}

impl Default for Sheet {
    fn default() -> Self {
        Sheet {
            background: "#ffffff".to_string(),
            radius: "24px".to_string(),
            shadow: true,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Card {
    pub background: String,
    /// A CSS length value. Use 999px for a pill shape.
    pub radius: String,
    pub shadow: bool,
    pub style: CardStyle,
}

impl Default for Card {
    fn default() -> Self {
        Card {
            background: "#f7fafc".to_string(),
            radius: "14px".to_string(),
            shadow: false,
            style: CardStyle::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CardStyle {
    /// Default: filled with the background color.
    #[default]
    Fill,
    /// Outline only.
    Outline,
    /// Semi-transparent + blurred.
    Glass,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct TextColors {
    pub heading: String,
    pub body: String,
    pub card_title: String,
    pub muted: String,
}

impl Default for TextColors {
    fn default() -> Self {
        TextColors {
            heading: "#111111".to_string(),
            body: "#333333".to_string(),
            card_title: "#111111".to_string(),
            muted: "#777777".to_string(),
        }
    }
}

/// Typography.
///
/// Picking a preset brings along both the font stack and the webfont CSS URL,
/// so you don't need to go hunt down a CDN URL yourself, and the editing UI
/// can turn it straight into a dropdown. For a font that isn't in the list,
/// use `preset = "custom"` together with `family` — the same pattern as
/// `platform = "custom"` + `icon` for social icons.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Font {
    /// Body font.
    pub preset: FontPreset,
    /// Heading font. Falls back to the body font if omitted.
    pub heading_preset: Option<FontPreset>,
    /// CSS font-family stack, required when `preset = "custom"`.
    pub family: Option<String>,
    /// Required when `heading_preset = "custom"`.
    pub heading_family: Option<String>,
    /// Extra webfont CSS URLs to inject **in addition to** whatever the preset provides.
    pub stylesheets: Vec<String>,
    pub heading_weight: u16,
    /// Base body size. All other sizes are computed relative to this.
    pub base_size: String,
    /// This is an f64. Using f32 makes 1.6 drift to 1.600000023841858 on
    /// every JSON round-trip, degrading the stored value each time it's saved.
    pub line_height: f64,
    /// A CSS `letter-spacing` value: `"normal"` or a length.
    pub letter_spacing: String,
}

impl Default for Font {
    fn default() -> Self {
        Font {
            preset: FontPreset::default(),
            heading_preset: None,
            family: None,
            heading_family: None,
            stylesheets: Vec::new(),
            heading_weight: 700,
            base_size: "15px".to_string(),
            line_height: 1.6,
            letter_spacing: "normal".to_string(),
        }
    }
}

impl Font {
    /// The value to put in `--pf-font-family`.
    ///
    /// Returns `None` if `preset` is `custom` and `family` is missing —
    /// validation catches that as an error, so the renderer never has to
    /// deal with it.
    pub fn body_family(&self) -> Option<&str> {
        match self.preset {
            FontPreset::Custom => self.family.as_deref(),
            preset => Some(preset.family()),
        }
    }

    /// The value to put in `--pf-font-heading-family`. Same as the body font
    /// when no heading preset is set.
    pub fn heading_family(&self) -> Option<&str> {
        match self.heading_preset {
            None => self.body_family(),
            Some(FontPreset::Custom) => self.heading_family.as_deref(),
            Some(preset) => Some(preset.family()),
        }
    }

    /// Webfont CSS URLs to put in `<head>`. Duplicates are removed — it's
    /// common for the body and heading to share a preset or CDN URL.
    pub fn stylesheet_urls(&self) -> Vec<&str> {
        let mut urls: Vec<&str> = Vec::new();

        let mut push = |url: Option<&'static str>| {
            if let Some(url) = url {
                if !urls.contains(&url) {
                    urls.push(url);
                }
            }
        };
        push(self.preset.stylesheet());
        if let Some(heading) = self.heading_preset {
            push(heading.stylesheet());
        }

        for url in &self.stylesheets {
            let url = url.as_str();
            if !urls.contains(&url) {
                urls.push(url);
            }
        }
        urls
    }
}

/// Built-in font presets.
///
/// To add a font, add a variant and fill in the three methods below. Like
/// `Platform`, this is meant to be an easy spot for contributors.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FontPreset {
    /// Uses the device's default font instead of downloading a webfont. Fastest option.
    #[default]
    System,
    Pretendard,
    NotoSansKr,
    NanumGothic,
    /// Serif (myeongjo).
    NanumMyeongjo,
    /// Handwriting style.
    Gaegu,
    /// Rounded, good for headings.
    Jua,
    IbmPlexSansKr,
    /// Clean gothic.
    GowunDodum,
    /// A font not in the list. Must be paired with `family`.
    Custom,
}

impl FontPreset {
    /// The **key** for the name shown in the editor UI dropdown.
    ///
    /// Returning the name directly would lock this function into one
    /// language. Font names are mostly proper nouns and stay as-is, but
    /// things like "device default" or "custom" need to be translated.
    pub fn label_key(self) -> &'static str {
        match self {
            FontPreset::System => "font.system",
            FontPreset::Pretendard => "font.pretendard",
            FontPreset::NotoSansKr => "font.notoSansKr",
            FontPreset::NanumGothic => "font.nanumGothic",
            FontPreset::NanumMyeongjo => "font.nanumMyeongjo",
            FontPreset::Gaegu => "font.gaegu",
            FontPreset::Jua => "font.jua",
            FontPreset::IbmPlexSansKr => "font.ibmPlexSansKr",
            FontPreset::GowunDodum => "font.gowunDodum",
            FontPreset::Custom => "font.custom",
        }
    }

    /// CSS font-family stack. Ends with the device default font as a
    /// fallback for when the webfont loads late or fails.
    pub fn family(self) -> &'static str {
        const FALLBACK: &str = "-apple-system, BlinkMacSystemFont, system-ui, \
             'Apple SD Gothic Neo', 'Malgun Gothic', sans-serif";

        match self {
            FontPreset::System => FALLBACK,
            FontPreset::Pretendard => {
                "'Pretendard Variable', Pretendard, -apple-system, BlinkMacSystemFont, \
                 system-ui, 'Apple SD Gothic Neo', 'Malgun Gothic', sans-serif"
            }
            FontPreset::NotoSansKr => {
                "'Noto Sans KR', -apple-system, BlinkMacSystemFont, system-ui, \
                 'Apple SD Gothic Neo', 'Malgun Gothic', sans-serif"
            }
            FontPreset::NanumGothic => {
                "'Nanum Gothic', -apple-system, system-ui, 'Malgun Gothic', sans-serif"
            }
            FontPreset::NanumMyeongjo => {
                "'Nanum Myeongjo', 'Apple SD Gothic Neo', 'Batang', serif"
            }
            FontPreset::Gaegu => "'Gaegu', 'Apple SD Gothic Neo', 'Malgun Gothic', cursive",
            FontPreset::Jua => "'Jua', 'Apple SD Gothic Neo', 'Malgun Gothic', sans-serif",
            FontPreset::IbmPlexSansKr => {
                "'IBM Plex Sans KR', -apple-system, system-ui, 'Malgun Gothic', sans-serif"
            }
            FontPreset::GowunDodum => {
                "'Gowun Dodum', -apple-system, system-ui, 'Malgun Gothic', sans-serif"
            }
            // Validation requires `family` here, so this branch is never reached.
            FontPreset::Custom => FALLBACK,
        }
    }

    /// Webfont CSS URL to put in `<head>`. The device default font has nothing to fetch.
    pub fn stylesheet(self) -> Option<&'static str> {
        match self {
            FontPreset::System | FontPreset::Custom => None,
            FontPreset::Pretendard => Some(
                "https://cdn.jsdelivr.net/gh/orioncactus/pretendard@v1.3.9/dist/web/variable/pretendardvariable-dynamic-subset.min.css",
            ),
            FontPreset::NotoSansKr => Some(
                "https://fonts.googleapis.com/css2?family=Noto+Sans+KR:wght@400;500;700;800&display=swap",
            ),
            FontPreset::NanumGothic => Some(
                "https://fonts.googleapis.com/css2?family=Nanum+Gothic:wght@400;700;800&display=swap",
            ),
            FontPreset::NanumMyeongjo => Some(
                "https://fonts.googleapis.com/css2?family=Nanum+Myeongjo:wght@400;700;800&display=swap",
            ),
            FontPreset::Gaegu => Some(
                "https://fonts.googleapis.com/css2?family=Gaegu:wght@300;400;700&display=swap",
            ),
            FontPreset::Jua => Some("https://fonts.googleapis.com/css2?family=Jua&display=swap"),
            FontPreset::IbmPlexSansKr => Some(
                "https://fonts.googleapis.com/css2?family=IBM+Plex+Sans+KR:wght@400;500;600;700&display=swap",
            ),
            FontPreset::GowunDodum => {
                Some("https://fonts.googleapis.com/css2?family=Gowun+Dodum&display=swap")
            }
        }
    }

    /// Weights actually provided. Using a weight not listed here makes the
    /// browser fake-bold the text (synthetic bold), which looks mushy.
    pub fn available_weights(self) -> &'static [u16] {
        match self {
            // Variable fonts — any value in the range works.
            FontPreset::Pretendard => &[100, 200, 300, 400, 500, 600, 700, 800, 900],
            FontPreset::NotoSansKr => &[400, 500, 700, 800],
            FontPreset::NanumGothic | FontPreset::NanumMyeongjo => &[400, 700, 800],
            FontPreset::Gaegu => &[300, 400, 700],
            FontPreset::IbmPlexSansKr => &[400, 500, 600, 700],
            // Fonts with only one weight available.
            FontPreset::Jua | FontPreset::GowunDodum => &[400],
            // Device fonts and custom fonts are unknown, so we don't check them.
            FontPreset::System | FontPreset::Custom => &[],
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Feature toggles / footer
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Features {
    /// The ⋮ button on link cards (copy link / OS share sheet).
    pub share_menu: bool,
    /// Adds a "save vCard (.vcf)" button at the top. Fitting for a business
    /// card — lets people add the contact straight to their address book.
    pub vcard_download: bool,
}

impl Default for Features {
    fn default() -> Self {
        Features {
            share_menu: true,
            vcard_download: true,
        }
    }
}

/// GitHub Pages deployment settings.
///
/// No token is accepted here — it uses whatever git credentials are already configured.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Deploy {
    /// Remote to push to.
    pub remote: String,
    /// Deployment branch. GitHub Pages needs to be configured to point at this branch.
    pub branch: String,
    /// Custom domain. If set, writes `dist/CNAME`.
    pub cname: Option<String>,
}

impl Default for Deploy {
    fn default() -> Self {
        Deploy {
            remote: "origin".to_string(),
            branch: "gh-pages".to_string(),
            cname: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Footer {
    pub text: Option<Text>,
    pub show_powered_by: bool,
}

impl Default for Footer {
    fn default() -> Self {
        Footer {
            text: None,
            show_powered_by: true,
        }
    }
}
