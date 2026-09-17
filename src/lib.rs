//! ProfileIT — 섹션 기반 온라인 명함을 만드는 정적 사이트 생성기.
//!
//! CLI(`src/main.rs`)와 데스크톱 앱(`src-tauri/`)이 이 라이브러리를 함께 씁니다.
//! 편집 로직을 앱 쪽에 두지 않은 이유는, 그러면 검증 규칙이 두 곳에 생기고
//! 둘이 어긋나기 시작하기 때문입니다.

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

/// 설정 파일이 놓인 폴더. 에셋 경로와 git 작업의 기준입니다.
///
/// `Path::parent` 는 `"profile.toml"` 같은 상대 경로에 대해 `None` 이 아니라
/// **빈 경로**를 돌려줍니다. 그대로 쓰면 `git` 이 빈 디렉터리를 보게 되어
/// "리포지터리가 아닙니다" 로 엉뚱하게 실패합니다.
pub fn project_root(config_path: &Path) -> &Path {
    config_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
}

/// 설정을 읽고 검증까지 마칩니다.
///
/// 오류가 하나라도 있으면 `Err` 입니다 — **검증에 실패한 설정으로는 빌드하지
/// 않습니다.** 깨진 결과물이 배포되는 것보다 빌드가 멈추는 쪽이 낫습니다.
/// 경고는 함께 돌려주므로 호출하는 쪽이 보여주면 됩니다.
pub fn load(path: &Path) -> Result<(Config, Vec<Diagnostic>), LoadError> {
    let source = std::fs::read_to_string(path).map_err(|err| LoadError::Read {
        path: path.display().to_string(),
        message: err.to_string(),
    })?;

    let config: Config = toml::from_str(&source).map_err(|err| LoadError::Parse {
        path: path.display().to_string(),
        // toml 크레이트가 줄·열 정보를 붙여주므로 그대로 넘깁니다.
        message: err.to_string(),
    })?;

    let root = project_root(path);
    let mut diagnostics = validate::validate(&config, root);
    // 섹션의 알 수 없는 키는 파싱에서 걸러지지 않아 원본을 한 번 더 훑습니다.
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
    /// 화면에 내보낼 문구. 어느 언어로 만들지는 부르는 쪽이 정합니다.
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

