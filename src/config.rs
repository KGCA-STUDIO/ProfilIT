//! `profile.toml` 의 스키마 정의.
//!
//! 이 모듈은 **편집 방식에 의존하지 않습니다.** 로컬 편집 UI, 정적 렌더러,
//! 나중에 붙일 브라우저 관리자 모드가 모두 같은 타입을 공유합니다. 새 필드를
//! 넣을 때는 `#[serde(default)]` 를 붙여 기존 파일이 계속 읽히도록 하고,
//! 호환되지 않는 변경에만 `SCHEMA_VERSION` 을 올리고 마이그레이션을 추가하세요.

use serde::{Deserialize, Serialize};

use crate::i18n::Text;

/// 이 바이너리가 읽을 수 있는 스키마 버전.
pub const SCHEMA_VERSION: u32 = 1;

/// 에셋 경로. `profile.toml` 이 있는 디렉터리 기준 상대 경로입니다.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(transparent)]
pub struct AssetPath(pub String);

fn default_true() -> bool {
    true
}

// ─────────────────────────────────────────────────────────────────────────────
// 루트
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema_version: u32,
    pub site: Site,
    pub profile: Profile,
    #[serde(default)]
    pub socials: Vec<Social>,
    /// 배열 순서가 화면 순서입니다.
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
    /// 화면에 실제로 렌더링될 섹션만 순서대로 돌려줍니다.
    pub fn visible_sections(&self) -> impl Iterator<Item = &Section> {
        self.sections.iter().filter(|s| s.enabled)
    }

    /// 생성할 언어들. **기본 언어가 항상 맨 앞**이고 중복은 제거됩니다.
    ///
    /// 맨 앞 언어가 사이트 최상단(`/`)에 놓이므로 순서가 곧 배치입니다.
    /// `languages` 에 기본 언어를 빠뜨리는 실수가 잦아서 여기서 보정합니다.
    pub fn languages(&self) -> Vec<String> {
        let mut langs = vec![self.site.lang.clone()];
        for lang in &self.site.languages {
            if !langs.contains(lang) {
                langs.push(lang.clone());
            }
        }
        langs
    }

    /// 기본 언어. 번역이 빠졌을 때 돌아갈 곳입니다.
    pub fn default_language(&self) -> &str {
        &self.site.lang
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 사이트 메타데이터
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Site {
    /// `<title>` 및 og:title.
    pub title: Text,
    #[serde(default)]
    pub description: Option<Text>,
    /// 배포 주소. og:url / canonical 을 절대 경로로 만들 때 필요합니다.
    #[serde(default)]
    pub base_url: Option<String>,
    /// 기본 언어. 이 언어가 사이트 최상단(`/`)에 놓입니다.
    #[serde(default = "default_lang")]
    pub lang: String,
    /// 생성할 언어 목록. 비우면 기본 언어 하나만 만듭니다.
    /// 기본 언어가 빠져 있으면 렌더러가 맨 앞에 넣습니다.
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
// 프로필 (명함 상단)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub name: Text,
    /// 이름 아래 한 줄. 직함이나 짧은 소개.
    #[serde(default)]
    pub tagline: Option<Text>,
    /// 여러 줄 허용. 렌더러가 줄바꿈을 `<br>` 로 변환합니다.
    #[serde(default)]
    pub bio: Option<Text>,
    #[serde(default)]
    pub avatar: Option<AssetPath>,
    /// "서울" 처럼 짧은 지역 표기.
    #[serde(default)]
    pub location: Option<Text>,
}

// ─────────────────────────────────────────────────────────────────────────────
// 소셜
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Social {
    pub platform: Platform,
    pub url: String,
    /// 스크린리더용 라벨. 생략하면 플랫폼 이름을 씁니다.
    #[serde(default)]
    pub label: Option<Text>,
    /// 직접 넣을 아이콘(SVG) 경로.
    ///
    /// 어느 플랫폼에나 쓸 수 있습니다 — 내장 글리프는 브랜드 마크를 흉내 낸
    /// 단순한 모양이라, 진짜 로고를 쓰고 싶으면 여기에 파일을 지정하면
    /// 그쪽이 이깁니다. `platform` 이 `custom` 이면 내장 글리프가 없으므로
    /// 필수입니다.
    #[serde(default)]
    pub icon: Option<AssetPath>,
}

/// 내장 아이콘이 있는 플랫폼 목록.
///
/// 아이콘을 추가하려면 여기에 변형을 넣고 `icons.rs` 에 SVG 를 등록하면 됩니다.
/// 기여자가 가장 쉽게 PR 할 수 있는 지점이라 일부러 열어둔 구조입니다.
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
    /// 내장 목록에 없는 경우. `icon` 필드가 함께 있어야 합니다.
    Custom,
}

impl Platform {
    /// locale 파일의 키. 표시 이름은 `locales/<코드>.json` 이 정합니다 —
    /// "네이버"/"Naver" 처럼 언어마다 달라져야 하기 때문입니다.
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
// 섹션
//
// 공통 필드(title·icon·enabled)를 바깥에 두고 `type` 별 본문만 flatten 으로
// 붙입니다. serde 는 `flatten` 과 `deny_unknown_fields` 를 함께 쓸 수 없어서
// 섹션에서는 오타를 파싱 단계에서 잡지 못합니다. 대신 `validate::lint_section_keys`
// 가 원본 TOML 을 다시 훑어 알 수 없는 키를 경고로 보고합니다.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Section {
    /// 섹션 제목. 생략하면 제목 없이 본문만 렌더링됩니다.
    #[serde(default)]
    pub title: Option<Text>,
    /// 제목 앞에 붙는 이모지 한 글자.
    #[serde(default)]
    pub icon: Option<String>,
    /// false 면 파일에는 남고 화면에서만 빠집니다.
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(flatten)]
    pub body: SectionBody,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SectionBody {
    /// 자유 문단. 자기소개, 하는 일.
    About { body: Text },
    /// 학력·경력처럼 시간순으로 쌓이는 항목.
    Timeline { items: Vec<TimelineItem> },
    /// 버킷리스트. 달성 여부를 체크합니다.
    Checklist {
        items: Vec<ChecklistItem>,
        /// 상단에 진행률 바를 표시합니다.
        #[serde(default = "default_true")]
        show_progress: bool,
    },
    /// 관심사·기술 같은 짧은 키워드 묶음.
    Tags { items: Vec<Tag> },
    /// 링크 카드 목록.
    Links { items: Vec<LinkItem> },
    /// 이메일·전화 같은 연락 수단.
    Contact { items: Vec<ContactItem> },
}

impl SectionBody {
    /// 해당 타입이 쓰는 본문 키 목록. 키 린트의 기준입니다.
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

    /// 모든 섹션이 공통으로 쓰는 키.
    pub const COMMON_KEYS: &'static [&'static str] = &["type", "title", "icon", "enabled"];
}

/// 관심사 태그.
///
/// 이모지가 필요 없으면 문자열(또는 번역 표) 그대로 씁니다.
///
/// ```toml
/// items = [
///   "Rust",
///   { ko = "요리", en = "Cooking" },
///   { icon = "🍳", text = { ko = "요리", en = "Cooking" } },
/// ]
/// ```
///
/// `WithIcon` 이 **먼저** 와야 합니다. 순서를 바꾸면 `{ icon = ..., text = ... }`
/// 가 언어 코드 `icon`·`text` 를 가진 번역 표로 잘못 읽힙니다.
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

/// 학력·경력 항목.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TimelineItem {
    pub title: Text,
    /// "2018 – 2022" 처럼 자유 형식입니다. 정렬에 쓰지 않으므로 형식을 강제하지
    /// 않습니다 — 배열 순서가 곧 표시 순서입니다.
    #[serde(default)]
    pub period: Option<Text>,
    /// 소속이나 학위 같은 부제.
    #[serde(default)]
    pub subtitle: Option<Text>,
    #[serde(default)]
    pub description: Option<Text>,
    /// 있으면 제목이 링크가 됩니다.
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

/// 버킷리스트 항목.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChecklistItem {
    pub text: Text,
    #[serde(default)]
    pub done: bool,
    /// 달성 시점. "2025-04" 처럼 자유 형식입니다.
    #[serde(default)]
    pub date: Option<Text>,
    /// 한 줄 후기.
    #[serde(default)]
    pub note: Option<Text>,
}

/// 링크 카드.
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
    /// 미세한 흔들림 애니메이션. `prefers-reduced-motion` 을 존중합니다.
    #[serde(default)]
    pub highlight: bool,
    /// NEW, 공지 같은 짧은 배지 텍스트.
    #[serde(default)]
    pub badge: Option<Text>,
}

/// 연락 수단.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContactItem {
    pub kind: ContactKind,
    /// 이메일 주소, 전화번호, 주소 문자열 등. 렌더러가 kind 에 맞는 링크로
    /// 감쌉니다(mailto:, tel:).
    pub value: Text,
    /// 화면에 보일 라벨. 생략하면 kind 의 기본 이름을 씁니다.
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
    /// 링크 없이 텍스트로만 표시합니다.
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
// 테마 — 아래 필드들은 CSS 커스텀 프로퍼티와 1:1 로 대응됩니다.
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Theme {
    pub background: Background,
    pub decoration: Option<Decoration>,
    /// 명함 본체(흰 종이).
    pub sheet: Sheet,
    /// 섹션 안에 들어가는 링크 카드.
    pub card: Card,
    pub text: TextColors,
    pub font: Font,
    /// 타임라인 점, 진행률 바, 태그 칩에 쓰이는 강조색. `--pf-accent`.
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

/// 배경.
///
/// `solid` · `gradient` · `pattern` 은 `--pf-background` 한 토큰으로 내려갑니다
/// (CSS `background` 단축 속성이 여러 레이어를 받으므로 패턴도 여기서 표현됩니다).
/// `image` 는 흐림 처리가 필요해서 전용 레이어(`.pf-backdrop`)를 씁니다 —
/// `background-image` 에는 `filter` 를 걸 수 없기 때문입니다.
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
    /// CSS 그라디언트로 그리는 반복 무늬. 이미지 파일이 필요 없습니다.
    Pattern {
        name: PatternName,
        /// 무늬 아래 바탕색.
        #[serde(default = "default_pattern_base")]
        color: String,
        /// 무늬 자체의 색.
        #[serde(default = "default_pattern_ink")]
        pattern_color: String,
        /// 무늬 한 칸의 크기. CSS 길이값.
        #[serde(default = "default_pattern_size")]
        size: String,
    },
    Image {
        src: AssetPath,
        #[serde(default)]
        fit: ImageFit,
        /// CSS `background-position` 값.
        #[serde(default = "default_image_position")]
        position: String,
        /// 흐림 반경. CSS 길이값. `"0px"` 이면 흐리지 않습니다.
        #[serde(default = "default_image_blur")]
        blur: String,
        /// 가독성을 위해 이미지 위에 덮는 반투명 색.
        #[serde(default)]
        overlay: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PatternName {
    /// 물방울 점.
    Dots,
    /// 모눈.
    Grid,
    /// 사선 줄무늬.
    Stripes,
    /// 체크무늬.
    Checks,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageFit {
    /// 화면을 꽉 채우고 넘치는 부분은 자릅니다.
    #[default]
    Cover,
    /// 이미지 전체가 보이도록 맞춥니다.
    Contain,
    /// 원본 크기로 타일처럼 반복합니다.
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

/// 색종이·스티커 장식 레이어. 순수 장식이라 `aria-hidden` 으로 나갑니다.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Decoration {
    /// 내장 SVG 세트.
    Preset { name: DecorationPreset },
    /// 상·하단에 직접 올리는 이미지.
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
    /// CSS 길이값.
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
    /// CSS 길이값. 알약 모양은 999px.
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
    /// 배경색으로 채운 기본형.
    #[default]
    Fill,
    /// 테두리만.
    Outline,
    /// 반투명 + 블러.
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

/// 글씨.
///
/// 프리셋을 고르면 폰트 스택과 웹폰트 CSS URL 이 함께 따라옵니다. CDN 주소를
/// 직접 찾아 넣지 않아도 되고, 편집 UI 에서는 그대로 드롭다운이 됩니다.
/// 목록에 없는 폰트는 `preset = "custom"` 에 `family` 를 함께 줍니다 —
/// 소셜 아이콘의 `platform = "custom"` + `icon` 과 같은 방식입니다.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Font {
    /// 본문 폰트.
    pub preset: FontPreset,
    /// 제목 폰트. 생략하면 본문과 같은 폰트를 씁니다.
    pub heading_preset: Option<FontPreset>,
    /// `preset = "custom"` 일 때 필수인 CSS font-family 스택.
    pub family: Option<String>,
    /// `heading_preset = "custom"` 일 때 필수.
    pub heading_family: Option<String>,
    /// 프리셋이 제공하는 URL 에 **더해서** 주입할 웹폰트 CSS URL.
    pub stylesheets: Vec<String>,
    pub heading_weight: u16,
    /// 본문 기준 크기. 이 값에서 나머지 크기가 상대적으로 계산됩니다.
    pub base_size: String,
    /// f64 입니다. f32 로 두면 JSON 왕복마다 1.6 이 1.600000023841858 로
    /// 번져서 저장할 때마다 값이 나빠집니다.
    pub line_height: f64,
    /// CSS `letter-spacing` 값. `"normal"` 또는 길이값.
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
    /// `--pf-font-family` 에 들어갈 값.
    ///
    /// `custom` 인데 `family` 가 없으면 `None` 입니다. 검증에서 오류로 잡히므로
    /// 렌더러는 여기까지 오지 않습니다.
    pub fn body_family(&self) -> Option<&str> {
        match self.preset {
            FontPreset::Custom => self.family.as_deref(),
            preset => Some(preset.family()),
        }
    }

    /// `--pf-font-heading-family` 에 들어갈 값. 제목 프리셋이 없으면 본문과 같습니다.
    pub fn heading_family(&self) -> Option<&str> {
        match self.heading_preset {
            None => self.body_family(),
            Some(FontPreset::Custom) => self.heading_family.as_deref(),
            Some(preset) => Some(preset.family()),
        }
    }

    /// `<head>` 에 넣을 웹폰트 CSS URL. 중복은 제거합니다 — 본문과 제목이 같은
    /// 프리셋이거나 같은 CDN 주소를 쓰는 경우가 흔합니다.
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

/// 내장 폰트 프리셋.
///
/// 폰트를 추가하려면 변형을 넣고 아래 세 메서드에 값을 채우면 됩니다.
/// `Platform` 과 마찬가지로 기여 받기 쉬운 지점입니다.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FontPreset {
    /// 웹폰트를 내려받지 않고 기기 기본 글꼴을 씁니다. 가장 빠릅니다.
    #[default]
    System,
    Pretendard,
    NotoSansKr,
    NanumGothic,
    /// 명조(세리프).
    NanumMyeongjo,
    /// 손글씨.
    Gaegu,
    /// 둥근 제목용.
    Jua,
    IbmPlexSansKr,
    /// 단정한 고딕.
    GowunDodum,
    /// 목록에 없는 폰트. `family` 를 함께 지정해야 합니다.
    Custom,
}

impl FontPreset {
    /// 편집 UI 드롭다운에 보일 이름의 **키**.
    ///
    /// 이름 자체를 돌려주면 이 함수가 언어를 정해버립니다. 폰트 이름은 고유명사
    /// 라 대부분 그대로지만 "기기 기본"·"직접 지정" 같은 것은 번역되어야 합니다.
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

    /// CSS font-family 스택. 웹폰트가 늦게 뜨거나 실패할 때를 대비해 뒤에
    /// 기기 기본 글꼴을 붙입니다.
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
            // 검증에서 family 를 요구하므로 여기까지 오지 않습니다.
            FontPreset::Custom => FALLBACK,
        }
    }

    /// `<head>` 에 넣을 웹폰트 CSS URL. 기기 기본 글꼴은 받을 것이 없습니다.
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

    /// 실제로 제공되는 굵기. 여기 없는 굵기를 쓰면 브라우저가 글자를 억지로
    /// 굵게 그려서(합성 볼드) 모양이 뭉개집니다.
    pub fn available_weights(self) -> &'static [u16] {
        match self {
            // 가변 폰트 — 구간 내 아무 값이나 됩니다.
            FontPreset::Pretendard => &[100, 200, 300, 400, 500, 600, 700, 800, 900],
            FontPreset::NotoSansKr => &[400, 500, 700, 800],
            FontPreset::NanumGothic | FontPreset::NanumMyeongjo => &[400, 700, 800],
            FontPreset::Gaegu => &[300, 400, 700],
            FontPreset::IbmPlexSansKr => &[400, 500, 600, 700],
            // 굵기가 하나뿐인 폰트.
            FontPreset::Jua | FontPreset::GowunDodum => &[400],
            // 기기 글꼴과 직접 지정은 알 수 없으므로 검사하지 않습니다.
            FontPreset::System | FontPreset::Custom => &[],
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 기능 토글 / 푸터
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Features {
    /// 링크 카드 오른쪽 ⋮ 버튼 (링크 복사 / OS 공유 시트).
    pub share_menu: bool,
    /// 상단에 vCard(.vcf) 저장 버튼을 넣습니다. 명함답게 연락처를 바로
    /// 주소록에 넣을 수 있습니다.
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

/// GitHub Pages 배포 설정.
///
/// 토큰은 받지 않습니다. 이미 설정된 git 자격증명을 그대로 씁니다.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields, default)]
pub struct Deploy {
    /// 올릴 리모트 이름.
    pub remote: String,
    /// 배포 브랜치. GitHub Pages 에서 이 브랜치를 가리키게 설정해야 합니다.
    pub branch: String,
    /// 사용자 지정 도메인. 있으면 `dist/CNAME` 을 씁니다.
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
