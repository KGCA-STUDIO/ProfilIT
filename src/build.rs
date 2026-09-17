//! `dist/` 를 만듭니다.
//!
//! 출력은 정적 파일 셋뿐입니다 — `index.html`, `styles.css`, `card.js`, 그리고
//! 설정이 참조하는 에셋. 서버가 필요 없고 GitHub Pages 에 그대로 올라갑니다.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::config::{AssetPath, Config, Decoration, SectionBody};
use crate::i18n::{self, Strings};
use crate::render::{self, Ctx};

/// 스타일시트와 스크립트는 바이너리에 박아 넣습니다. 설치한 사람이 파일을
/// 따로 챙기지 않아도 되고, 버전이 어긋날 일도 없습니다.
const STYLES: &str = include_str!("../static/styles.css");
const SCRIPT: &str = include_str!("../static/card.js");

pub struct Output {
    pub html_bytes: usize,
    pub assets_copied: usize,
    pub languages: Vec<String>,
}

pub fn build(config: &Config, root: &Path, dist: &Path) -> io::Result<Output> {
    // 이전 결과가 남아 섞이지 않도록 비우고 시작합니다. 지우는 범위는 우리가
    // 만든 dist 뿐이고, 없으면 그냥 넘어갑니다.
    if dist.exists() {
        fs::remove_dir_all(dist)?;
    }
    fs::create_dir_all(dist)?;

    let languages = config.languages();
    let default_lang = config.default_language();

    // 선택기에 넣을 (코드, 표시 이름, 경로). 기본 언어가 루트에 놓입니다.
    let entries: Vec<(String, String, String)> = languages
        .iter()
        .map(|lang| {
            let path = if lang == default_lang {
                String::new()
            } else {
                format!("{lang}/")
            };
            (lang.clone(), i18n::language_name(lang, root), path)
        })
        .collect();

    let mut html_bytes = 0;
    for lang in &languages {
        let strings = Strings::load(lang, root)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

        let is_default = lang == default_lang;
        let ctx = Ctx {
            config,
            lang,
            fallback: default_lang,
            strings: &strings,
            languages: &entries,
            prefix: if is_default { "" } else { "../" },
        };

        let html = render::page(&ctx);
        html_bytes += html.len();

        let dir = if is_default {
            dist.to_path_buf()
        } else {
            dist.join(lang)
        };
        fs::create_dir_all(&dir)?;
        fs::write(dir.join("index.html"), &html)?;
    }

    // 스타일시트·스크립트·에셋은 루트에 한 벌만 두고 모든 언어판이 공유합니다.
    fs::write(dist.join("styles.css"), STYLES)?;

    // 버튼이 하나도 없으면 스크립트를 받을 이유가 없습니다.
    if config.features.share_menu || config.features.vcard_download {
        fs::write(dist.join("card.js"), SCRIPT)?;
    }

    let mut assets_copied = 0;
    for asset in collect_assets(config) {
        let source = root.join(&asset.0);
        let target = dist.join(&asset.0);

        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&source, &target)?;
        assets_copied += 1;
    }

    Ok(Output {
        html_bytes,
        assets_copied,
        languages,
    })
}

/// 설정이 참조하는 모든 파일. 검증에서 존재가 확인된 것들입니다.
///
/// 새 에셋 필드를 스키마에 넣으면 **여기에도 추가해야 합니다.** 빠뜨리면
/// 로컬에서는 멀쩡하고 배포한 곳에서만 그림이 깨집니다.
fn collect_assets(config: &Config) -> Vec<&AssetPath> {
    let mut assets: Vec<&AssetPath> = Vec::new();

    assets.extend(config.site.og_image.as_ref());
    assets.extend(config.site.favicon.as_ref());
    assets.extend(config.profile.avatar.as_ref());

    for social in &config.socials {
        assets.extend(social.icon.as_ref());
    }

    for section in config.visible_sections() {
        if let SectionBody::Links { items } = &section.body {
            for item in items.iter().filter(|i| i.enabled) {
                assets.extend(item.thumbnail.as_ref());
            }
        }
    }

    if let crate::config::Background::Image { src, .. } = &config.theme.background {
        assets.push(src);
    }
    if let Some(Decoration::Custom { top, bottom }) = &config.theme.decoration {
        assets.extend(top.as_ref());
        assets.extend(bottom.as_ref());
    }

    // 같은 파일을 두 곳에서 쓰면 한 번만 복사합니다.
    let mut seen = Vec::new();
    assets.retain(|asset| {
        if seen.contains(&asset.0.as_str()) {
            false
        } else {
            seen.push(asset.0.as_str());
            true
        }
    });

    assets
}

/// 기본 출력 경로. 설정 파일이 있는 곳 옆에 만듭니다.
pub fn default_dist(config_path: &Path) -> PathBuf {
    crate::project_root(config_path).join("dist")
}
