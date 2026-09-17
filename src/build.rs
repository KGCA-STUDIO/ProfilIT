//! Builds `dist/`.
//!
//! The output is just a set of static files — `index.html`, `styles.css`,
//! `card.js`, plus whatever assets the config references. No server needed;
//! it can be dropped straight onto GitHub Pages.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::config::{AssetPath, Config, Decoration, SectionBody};
use crate::i18n::{self, Strings};
use crate::render::{self, Ctx};

/// The stylesheet and script are embedded straight into the binary, so
/// whoever installs this doesn't need to keep the files around separately,
/// and there's no risk of the version drifting out of sync.
const STYLES: &str = include_str!("../static/styles.css");
const SCRIPT: &str = include_str!("../static/card.js");

pub struct Output {
    pub html_bytes: usize,
    pub assets_copied: usize,
    pub languages: Vec<String>,
}

pub fn build(config: &Config, root: &Path, dist: &Path) -> io::Result<Output> {
    // Start clean so leftovers from a previous build don't get mixed in. We
    // only ever remove the dist dir we created, and skip this if it's absent.
    if dist.exists() {
        fs::remove_dir_all(dist)?;
    }
    fs::create_dir_all(dist)?;

    let languages = config.languages();
    let default_lang = config.default_language();

    // (code, display name, path) entries for the language selector. The
    // default language lives at the root.
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

    // The stylesheet, script, and assets live in one copy at the root and are
    // shared by every language version.
    fs::write(dist.join("styles.css"), STYLES)?;

    // No point shipping the script if there's no button that needs it.
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

/// Every file the config references. Validation has already confirmed these exist.
///
/// If you add a new asset field to the schema, **you must add it here too.**
/// Miss this and it'll work fine locally but images will break wherever it's deployed.
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

    // If the same file is used in two places, only copy it once.
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

/// The default output path, created next to the config file.
pub fn default_dist(config_path: &Path) -> PathBuf {
    crate::project_root(config_path).join("dist")
}
