//! ProfileIT — a static site generator for section-based online business cards.
//!
//! Both the CLI (`src/main.rs`) and the desktop app (`src-tauri/`) share this
//! library. The editing logic doesn't live in the app itself, because that
//! would create two separate sets of validation rules that would eventually
//! drift apart.

pub mod build;
pub mod config;
pub mod decor;
pub mod deploy;
pub mod i18n;
pub mod icons;
pub mod init;
pub mod message;
pub mod render;
pub mod theme;
pub mod validate;

#[cfg(feature = "editor")]
pub mod github;
#[cfg(feature = "editor")]
pub mod save;
#[cfg(feature = "editor")]
pub mod serve;

use std::path::Path;

use config::Config;
use message::Message;
use validate::Diagnostic;

/// The folder the config file lives in. This is the base for asset paths and git operations.
///
/// `Path::parent` returns an **empty path** rather than `None` for a relative
/// path like `"profile.toml"`. Using that as-is would make `git` see an empty
/// directory and fail with a confusing "not a repository" error.
pub fn project_root(config_path: &Path) -> &Path {
    config_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

/// Reads the config and runs it through validation.
///
/// Returns `Err` if there's even a single error — **we never build from a
/// config that failed validation.** A stopped build beats a broken result
/// getting deployed. Warnings are returned alongside, for the caller to display.
pub fn load(path: &Path) -> Result<(Config, Vec<Diagnostic>), LoadError> {
    let source = std::fs::read_to_string(path).map_err(|err| LoadError::Read {
        path: path.display().to_string(),
        message: err.to_string(),
    })?;

    let config: Config = toml::from_str(&source).map_err(|err| LoadError::Parse {
        path: path.display().to_string(),
        // The toml crate already attaches line/column info, so we just pass it through.
        message: err.to_string(),
    })?;

    let root = project_root(path);
    let mut diagnostics = validate::validate(&config, root);
    // Unknown section keys slip past parsing, so we re-scan the raw source for them.
    diagnostics.extend(validate::lint_section_keys(&source));

    if validate::has_errors(&diagnostics) {
        return Err(LoadError::Invalid(diagnostics));
    }

    Ok((config, diagnostics))
}

#[derive(Debug)]
pub enum LoadError {
    Read { path: String, message: String },
    Parse { path: String, message: String },
    Invalid(Vec<Diagnostic>),
}

impl LoadError {
    /// The message to display. Whichever side calls this decides the language.
    pub fn message(&self) -> Message {
        match self {
            LoadError::Read { path, message } => Message::new("msg.load.read")
                .with("path", path)
                .with("detail", message),
            LoadError::Parse { path, message } => Message::new("msg.load.parse")
                .with("path", path)
                .with("detail", message),
            LoadError::Invalid(diagnostics) => {
                let errors = diagnostics
                    .iter()
                    .filter(|d| d.severity == validate::Severity::Error)
                    .count();
                Message::new("msg.load.invalid").with("count", errors)
            }
        }
    }
}

