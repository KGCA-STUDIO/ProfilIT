//! 로컬 편집 UI.
//!
//! `127.0.0.1` 에만 붙는 작은 axum 서버입니다. 왼쪽은 폼, 오른쪽은 **실제
//! 렌더러 출력**을 담은 iframe 이라 미리보기가 결과물과 어긋날 수 없습니다.
//! 편집기용 렌더링을 따로 만들었다면 둘이 조금씩 달라졌을 겁니다.
//!
//! 인증은 없습니다. 루프백에만 바인딩하고, 다른 기기에서 접근할 수 없습니다.

use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::extract::{Query, State};
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::config::{Config, FontPreset};
use crate::i18n::{self, Strings};
use crate::message::Message;
use crate::render::{self, Ctx};
use crate::{build, deploy, github, save, validate};

const EDITOR_HTML: &str = include_str!("../static/editor.html");
const EDITOR_CSS: &str = include_str!("../static/editor.css");
const EDITOR_JS: &str = include_str!("../static/editor.js");
const STYLES: &str = include_str!("../static/styles.css");
const SCRIPT: &str = include_str!("../static/card.js");

struct AppState {
    config_path: PathBuf,
    root: PathBuf,
}

type Shared = Arc<Mutex<AppState>>;

/// 떠 있는 편집 서버로 가는 손잡이.
///
/// 데스크톱 앱이 **서버를 다시 띄우지 않고** 다른 명함 폴더로 옮겨갈 수 있게
/// 합니다. 같은 프로세스 안이라 가능한 일입니다 — 자식 프로세스였다면 죽이고
/// 새로 띄우고 포트를 다시 기다려야 합니다.
#[derive(Clone)]
pub struct Editor {
    url: String,
    state: Shared,
}

impl Editor {
    pub fn url(&self) -> &str {
        &self.url
    }

    /// 지금 열려 있는 설정 파일.
    pub fn config_path(&self) -> PathBuf {
        self.state.lock().unwrap().config_path.clone()
    }

    /// 다른 명함으로 갈아탑니다. 창은 호출하는 쪽이 새로고침하면 됩니다.
    pub fn open(&self, config_path: &Path) {
        let root = crate::project_root(config_path).to_path_buf();

        let mut state = self.state.lock().unwrap();
        state.config_path = config_path.to_path_buf();
        state.root = root;
    }
}

/// 터미널에서 띄웁니다. Ctrl+C 를 누를 때까지 돌아오지 않습니다.
pub fn run(config_path: &Path) -> std::io::Result<()> {
    let app = router(new_state(config_path));

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    // 터미널에 쓸 언어. 편집기 화면 언어와 따로입니다 — 이 줄들은 편집기가
    // 뜨기 전에 찍히고, 그때는 화면이 무슨 언어를 고를지 알 수 없습니다.
    let strings = terminal_strings(config_path);
    // 종료 인사는 미리 만들어 둡니다. 셧다운 future 는 `strings` 보다 오래 살 수
    // 있어서 참조를 들고 들어갈 수 없습니다.
    let closed = strings.get("msg.serve.closed").to_string();

    runtime.block_on(async move {
        let (listener, addr) = bind().await?;
        let url = format!("http://{addr}/");

        // 데스크톱 앱이 읽는 줄. 사람이 읽는 문구는 번역될 수 있으므로
        // 기계가 찾을 표식을 따로 둡니다.
        println!("PROFILEIT_LISTENING {url}");
        println!(
            "{}",
            Message::new("msg.serve.opened")
                .with("url", &url)
                .render(&strings)
        );
        println!("{}", strings.get("msg.serve.stop"));

        // 데스크톱 앱 안에서는 창이 이미 있으므로 브라우저를 또 열지 않습니다.
        if std::env::var_os("PROFILEIT_NO_BROWSER").is_none() {
            if !open_browser(&url) {
                println!(
                    "{}",
                    Message::new("msg.serve.notOpenable")
                        .with("url", &url)
                        .render(&strings)
                );
            }
        }

        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = tokio::signal::ctrl_c().await;
                println!();
                println!("{closed}");
            })
            .await
    })
}

/// 같은 프로세스 안에서 띄우고 주소를 돌려줍니다.
///
/// 데스크톱 앱(`src-tauri/`)이 씁니다. 둘 다 Rust 라서 자식 프로세스를 띄우고
/// 포트를 넘기고 뜰 때까지 기다리는 과정이 통째로 필요 없습니다 — 앱이 끝나면
/// 서버도 같이 끝나므로 고아 프로세스가 남지도 않습니다.
pub fn spawn(config_path: &Path) -> std::io::Result<Editor> {
    let state = new_state(config_path);
    let app = router(state.clone());
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::Builder::new()
        .name("profileit-editor".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(err) => {
                    let _ = tx.send(Err(err));
                    return;
                }
            };

            runtime.block_on(async move {
                let (listener, addr) = match bind().await {
                    Ok(pair) => pair,
                    Err(err) => {
                        let _ = tx.send(Err(err));
                        return;
                    }
                };

                // 주소를 먼저 알려야 창이 뜰 수 있습니다.
                if tx.send(Ok(format!("http://{addr}/"))).is_err() {
                    return; // 기다리던 쪽이 사라졌으면 띄울 이유가 없습니다.
                }
                let _ = axum::serve(listener, app).await;
            });
        })?;

    let url = rx.recv().map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::Other, "편집 서버 스레드가 멈췄습니다")
    })??;

    Ok(Editor { url, state })
}

/// 터미널에 쓸 언어. `PROFILEIT_LANG` 이 있으면 그것을, 없으면 이 명함의
/// 기본 언어를 씁니다 — CLI(`src/main.rs`)와 같은 규칙입니다.
fn terminal_strings(config_path: &Path) -> Strings {
    let root = crate::project_root(config_path);
    let chosen = std::env::var("PROFILEIT_LANG").ok().unwrap_or_else(|| {
        crate::load(config_path)
            .map(|(config, _)| config.default_language().to_string())
            .unwrap_or_else(|_| "ko".to_string())
    });

    Strings::load(&chosen, root)
        .or_else(|_| Strings::load("ko", root))
        .unwrap_or_default()
}

fn new_state(config_path: &Path) -> Shared {
    let root = crate::project_root(config_path).to_path_buf();

    Arc::new(Mutex::new(AppState {
        config_path: config_path.to_path_buf(),
        root,
    }))
}

/// 라우터. `run` 과 `spawn` 이 공유합니다.
///
/// 루프백에만 바인딩합니다. 0.0.0.0 이면 같은 네트워크의 다른 기기가 설정을
/// 고칠 수 있게 되는데, 인증이 없으므로 그건 곤란합니다.
fn router(state: Shared) -> Router {
    Router::new()
        .route("/", get(|| async { Html(EDITOR_HTML) }))
        .route("/editor.css", get(|| async { css(EDITOR_CSS) }))
        .route("/editor.js", get(|| async { js(EDITOR_JS) }))
        .route("/api/config", get(get_config).post(post_config))
        .route("/api/build", post(post_build))
        .route("/api/deploy", post(post_deploy))
        .route("/api/github", get(get_github))
        .route("/api/github/connect", post(post_github_connect))
        .route("/api/github/disconnect", post(post_github_disconnect))
        .route("/api/github/repo", post(post_github_repo))
        .route("/api/open", post(post_open))
        .route("/api/upload", post(post_upload))
        .route("/preview/", get(preview))
        .route("/preview/styles.css", get(|| async { css(STYLES) }))
        .route("/preview/card.js", get(|| async { js(SCRIPT) }))
        .route("/preview/{*path}", get(preview_asset))
        .with_state(state)
}

/// 기본 포트가 쓰이고 있으면 몇 개 더 시도합니다. 편집기를 두 개 띄우는 일이
/// 드물지 않은데, 그때마다 포트를 직접 고르게 하고 싶지 않습니다.
///
/// `PROFILEIT_PORT` 가 있으면 그 포트만 씁니다. 데스크톱 앱이 빈 포트를 먼저
/// 잡아두고 넘겨주기 때문에, 다른 포트로 흘러가면 창이 엉뚱한 곳을 봅니다.
async fn bind() -> std::io::Result<(tokio::net::TcpListener, SocketAddr)> {
    if let Some(port) = std::env::var("PROFILEIT_PORT").ok().and_then(|p| p.parse().ok()) {
        let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        return tokio::net::TcpListener::bind(addr).await.map(|l| (l, addr));
    }

    let mut last_err = None;
    for port in 4180..4190 {
        let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        match tokio::net::TcpListener::bind(addr).await {
            Ok(listener) => return Ok((listener, addr)),
            Err(err) => last_err = Some(err),
        }
    }
    Err(last_err.unwrap_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::AddrInUse, "빈 포트를 찾지 못했습니다")
    }))
}

/// 기본 브라우저로 주소를 엽니다.
///
/// **셸을 거치지 않습니다.** 윈도에서 `cmd /C start` 를 쓰면 cmd 가 명령줄을
/// 자기 규칙으로 다시 해석해서, 주소에 들어 있는 `&`·`|`·`%` 가 명령 구분자나
/// 환경 변수 확장으로 동작합니다. `https://example.com/?a=1&calc` 같은 주소
/// 하나로 다른 프로그램이 실행될 수 있습니다.
///
/// `rundll32 url.dll,FileProtocolHandler` 는 셸을 타지 않고 등록된 기본
/// 브라우저로 바로 넘깁니다. 유닉스에서는 `Command` 가 원래 셸을 쓰지 않으므로
/// `open`·`xdg-open` 을 그대로 부르면 됩니다.
fn open_browser(url: &str) -> bool {
    if !is_openable(url) {
        // 문구는 부르는 쪽이 자기 언어로 냅니다. 여기서는 열었는지만 알립니다.
        return false;
    }

    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .spawn();
    #[cfg(target_os = "macos")]
    let result = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let result = std::process::Command::new("xdg-open").arg(url).spawn();

    // 브라우저가 안 열려도 주소를 출력했으니 치명적이지 않습니다.
    result.is_ok()
}

/// 넘겨도 되는 주소인지.
///
/// 셸을 안 타므로 특수문자 자체는 위험하지 않지만, 정상적인 주소에는 공백이나
/// 제어문자가 들어가지 않습니다. 들어 있다면 어딘가에서 조작된 값입니다.
fn is_openable(url: &str) -> bool {
    let http = url.starts_with("http://") || url.starts_with("https://");
    let clean = !url.chars().any(|c| c.is_control() || c.is_whitespace());
    http && clean
}

// ─────────────────────────────────────────────────────────────────────────────
// API
// ─────────────────────────────────────────────────────────────────────────────

/// 편집기로 보내는 진단.
///
/// 문장이 아니라 키와 인자를 보냅니다. 편집기가 자기 화면 언어로 조립하므로,
/// 서버가 어느 언어를 쓰는지와 무관하게 읽을 수 있습니다.
#[derive(Serialize)]
struct Diagnostic {
    severity: &'static str,
    path: String,
    key: &'static str,
    args: std::collections::BTreeMap<&'static str, String>,
}

fn to_json_diagnostics(diagnostics: &[validate::Diagnostic]) -> Vec<Diagnostic> {
    diagnostics
        .iter()
        .map(|d| Diagnostic {
            severity: match d.severity {
                validate::Severity::Error => "error",
                validate::Severity::Warning => "warning",
            },
            path: d.path.clone(),
            key: d.message.key,
            args: d.message.args.clone(),
        })
        .collect()
}

async fn get_config(State(state): State<Shared>) -> Response {
    let (path, root) = {
        let state = state.lock().unwrap();
        (state.config_path.clone(), state.root.clone())
    };

    let source = match std::fs::read_to_string(&path) {
        Ok(source) => source,
        Err(err) => {
            return error_json(
                Message::new("msg.load.read")
                    .with("path", path.display())
                    .with("detail", err),
            )
        }
    };

    let config: Config = match toml::from_str(&source) {
        Ok(config) => config,
        Err(err) => {
            return error_json(
                Message::new("msg.load.parse")
                    .with("path", path.display())
                    .with("detail", err),
            )
        }
    };

    let mut diagnostics = validate::validate(&config, &root);
    diagnostics.extend(validate::lint_section_keys(&source));

    Json(json!({
        "config": config,
        "diagnostics": to_json_diagnostics(&diagnostics),
        "meta": meta(&root),
    }))
    .into_response()
}

/// 편집기가 드롭다운을 채우는 데 쓰는 목록들.
///
/// 하드코딩하지 않고 서버가 넘기는 이유: 폰트 프리셋이나 언어를 늘렸을 때
/// 편집기 JS 를 따라 고치는 것을 잊기 쉽습니다.
fn meta(root: &Path) -> Value {
    let fonts: Vec<Value> = [
        FontPreset::System,
        FontPreset::Pretendard,
        FontPreset::NotoSansKr,
        FontPreset::NanumGothic,
        FontPreset::NanumMyeongjo,
        FontPreset::Gaegu,
        FontPreset::Jua,
        FontPreset::IbmPlexSansKr,
        FontPreset::GowunDodum,
        FontPreset::Custom,
    ]
    .iter()
    .map(|preset| {
        let code = serde_json::to_value(preset).unwrap_or(Value::Null);
        json!({
            "value": code,
            "labelKey": preset.label_key(),
            "weights": preset.available_weights(),
        })
    })
    .collect();

    let languages: Vec<Value> = i18n::builtin_codes()
        .iter()
        .map(|code| json!({ "value": code, "label": i18n::language_name(code, root) }))
        .collect();

    // 편집기 화면 문구를 언어별로 한 번에 내려줍니다. 화면 언어를 바꿀 때
    // 서버에 다시 묻지 않고 즉시 갈아끼울 수 있습니다.
    //
    // `msg.` 도 함께 보냅니다. 서버가 보내는 오류·경고는 완성된 문장이 아니라
    // 키라서, 편집기가 자기 표를 갖고 있어야 화면 언어로 읽을 수 있습니다.
    let mut ui_strings = serde_json::Map::new();
    for code in i18n::builtin_codes() {
        if let Ok(strings) = Strings::load(code, root) {
            let mut table = serde_json::Map::new();
            for prefix in ["editor.", "msg.", "font."] {
                for (key, value) in strings.with_prefix(prefix) {
                    table.insert(key.to_string(), Value::String(value.to_string()));
                }
            }
            ui_strings.insert(code.to_string(), Value::Object(table));
        }
    }

    json!({
        "fonts": fonts,
        "languages": languages,
        "uiStrings": ui_strings,
        "platforms": [
            "instagram", "youtube", "threads", "tiktok", "x", "facebook",
            "naver", "naver_blog", "kakao_talk", "github", "linkedin",
            "email", "rss", "custom",
        ],
        "sectionTypes": ["about", "timeline", "checklist", "tags", "links", "contact"],
        "contactKinds": ["email", "phone", "address", "website", "custom"],
        "backgroundTypes": ["solid", "gradient", "pattern", "image"],
        "patternNames": ["dots", "grid", "stripes", "checks"],
        "decorationPresets": ["confetti", "sparkle", "bubble"],
        "cardStyles": ["fill", "outline", "glass"],
        "imageFits": ["cover", "contain", "repeat"],
    })
}

async fn post_config(State(state): State<Shared>, Json(body): Json<Value>) -> Response {
    let (path, root) = {
        let state = state.lock().unwrap();
        (state.config_path.clone(), state.root.clone())
    };

    // 먼저 스키마에 맞는지 봅니다. 여기서 실패하면 파일은 건드리지 않습니다.
    let config: Config = match serde_json::from_value(body.clone()) {
        Ok(config) => config,
        Err(err) => {
            return error_json(Message::new("msg.api.badShape").with("detail", err))
        }
    };

    let diagnostics = validate::validate(&config, &root);
    if validate::has_errors(&diagnostics) {
        // 오류가 있으면 저장하지 않습니다 — 깨진 설정이 파일에 남는 것보다
        // 편집기에서 고치는 쪽이 낫습니다.
        return Json(json!({
            "saved": false,
            "diagnostics": to_json_diagnostics(&diagnostics),
        }))
        .into_response();
    }

    match save::save(&path, &body) {
        Err(err) => error_json(Message::new("msg.api.saveFailed").with("detail", err)),
        Ok(_) => Json(json!({
            "saved": true,
            "diagnostics": to_json_diagnostics(&diagnostics),
        }))
        .into_response(),
    }
}

/// 편집기의 "사이트 생성" 버튼. 저장된 설정으로 `dist/` 를 만듭니다.
async fn post_build(State(state): State<Shared>) -> Response {
    let (path, root) = {
        let state = state.lock().unwrap();
        (state.config_path.clone(), state.root.clone())
    };

    let source = match std::fs::read_to_string(&path) {
        Ok(source) => source,
        Err(err) => {
            return error_json(
                Message::new("msg.load.read")
                    .with("path", path.display())
                    .with("detail", err),
            )
        }
    };
    let config: Config = match toml::from_str(&source) {
        Ok(config) => config,
        Err(err) => {
            return error_json(
                Message::new("msg.load.parse")
                    .with("path", path.display())
                    .with("detail", err),
            )
        }
    };

    let dist = build::default_dist(&path);
    match build::build(&config, &root, &dist) {
        Err(err) => error_json(Message::new("msg.api.buildFailed").with("detail", err)),
        Ok(output) => Json(json!({
            "dist": dist.display().to_string(),
            "languages": output.languages,
            "assets": output.assets_copied,
        }))
        .into_response(),
    }
}

#[derive(Deserialize)]
struct OpenBody {
    url: String,
}

/// 주소를 기본 브라우저로 엽니다.
///
/// 데스크톱 앱의 웹뷰는 `window.open` 을 막습니다. 막지 않더라도 편집기 창이
/// 외부 사이트로 넘어가 버리면 돌아올 방법이 마땅치 않습니다. 그래서 서버가
/// 대신 엽니다 — 앱에서도 일반 브라우저에서도 같은 코드로 동작합니다.
async fn post_open(Json(body): Json<OpenBody>) -> Response {
    let url = body.url.trim();

    // 판정은 `is_openable` 한 곳에서만 합니다. 여기서 따로 검사하면 둘이
    // 어긋나서, 열리지도 않았는데 열렸다고 답하는 일이 생깁니다.
    if !is_openable(url) {
        return error_json(Message::new("msg.api.notOpenable"));
    }

    open_browser(url);
    Json(json!({ "opened": true })).into_response()
}

// ─────────────────────────────────────────────────────────────────────────────
// GitHub 연결
// ─────────────────────────────────────────────────────────────────────────────

/// 연결 상태. 편집기가 화면을 그리는 데 필요한 것을 한 번에 돌려줍니다.
async fn get_github(State(state): State<Shared>) -> Response {
    let root = state.lock().unwrap().root.clone();

    let Some(token) = github::load() else {
        return Json(json!({
            "connected": false,
            "tokenPageUrl": github::token_page_url(),
            "scopes": github::SCOPES,
            "remote": current_remote(&root),
        }))
        .into_response();
    };

    // 토큰이 만료됐거나 취소됐을 수 있습니다. 목록을 받아보기 전에 확인합니다.
    let account = match github::whoami(&token) {
        Ok(account) => account,
        Err(err) => {
            return Json(json!({
                "connected": false,
                "error": { "key": err.message().key, "args": err.message().args },
                "tokenPageUrl": github::token_page_url(),
                "scopes": github::SCOPES,
                "remote": current_remote(&root),
            }))
            .into_response()
        }
    };

    let repos = github::list_repos(&token).unwrap_or_default();

    Json(json!({
        "connected": true,
        "account": account,
        "repos": repos,
        "remote": current_remote(&root),
    }))
    .into_response()
}

/// 지금 프로젝트가 가리키는 리모트. 어느 저장소가 골라져 있는지 보여줍니다.
fn current_remote(root: &std::path::Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .current_dir(root)
        .args(["remote", "get-url", "origin"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }
    let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!url.is_empty()).then_some(url)
}

#[derive(Deserialize)]
struct ConnectBody {
    token: String,
}

async fn post_github_connect(Json(body): Json<ConnectBody>) -> Response {
    let token = deploy::Token::new(body.token.trim());

    // 저장하기 전에 써봅니다. 잘못된 토큰을 저장소에 넣어두면 다음 실행에서
    // 연결된 것처럼 보이다가 실제 작업에서 실패합니다.
    let account = match github::whoami(&token) {
        Ok(account) => account,
        Err(err) => return error_json(err.message()),
    };

    if let Err(err) = github::store(&token) {
        return error_json(err.message());
    }

    Json(json!({ "account": account })).into_response()
}

async fn post_github_disconnect() -> Response {
    match github::forget() {
        Err(err) => error_json(err.message()),
        Ok(()) => Json(json!({ "disconnected": true })).into_response(),
    }
}

#[derive(Deserialize)]
struct RepoBody {
    /// 기존 저장소를 고른 경우.
    #[serde(default)]
    clone_url: Option<String>,
    /// 새로 만드는 경우.
    #[serde(default)]
    create: Option<String>,
    #[serde(default)]
    private: bool,
}

/// 저장소를 고르거나 만들고, 이 폴더의 리모트를 거기로 맞춥니다.
async fn post_github_repo(State(state): State<Shared>, Json(body): Json<RepoBody>) -> Response {
    let root = state.lock().unwrap().root.clone();

    let url = match (&body.clone_url, &body.create) {
        (Some(url), _) => url.clone(),

        (None, Some(name)) => {
            let Some(token) = github::load() else {
                return error_json(Message::new("msg.api.connectFirst"));
            };
            match github::create_repo(&token, name.trim(), body.private) {
                Ok(repo) => repo.clone_url,
                Err(err) => return error_json(err.message()),
            }
        }

        (None, None) => return error_json(Message::new("msg.api.pickRepo")),
    };

    if let Err(err) = deploy::set_remote(&root, "origin", &url) {
        return error_json(err.message());
    }

    Json(json!({ "remote": url })).into_response()
}

/// 편집기의 "GitHub 배포" 버튼.
///
/// 올리기 전에 반드시 다시 빌드합니다 — 고친 내용이 빠진 dist 를 올리는 것이
/// 가장 흔한 실수입니다.
async fn post_deploy(State(state): State<Shared>) -> Response {
    let (path, root) = {
        let state = state.lock().unwrap();
        (state.config_path.clone(), state.root.clone())
    };

    let (config, _) = match crate::load(&path) {
        Ok(loaded) => loaded,
        Err(err) => return error_json(err.message()),
    };

    let dist = build::default_dist(&path);
    if let Err(err) = build::build(&config, &root, &dist) {
        return error_json(Message::new("msg.api.buildFailed").with("detail", err));
    }

    let token = github::load();
    match deploy::publish(&config, &root, &dist, token.as_ref()) {
        Err(err) => error_json(err.message()),
        Ok(mut outcome) => {
            // 올린 뒤 Pages 를 켭니다. 이게 없으면 푸시는 됐는데 주소가
            // 404 인 상태로 남아, 무엇이 빠졌는지 알기 어렵습니다.
            if let Some(token) = &token {
                match pages_for(&config, &root, token, &outcome.branch) {
                    Ok(Some(url)) => outcome.pages_url = Some(url),
                    Ok(None) => {}
                    Err(message) => outcome.warnings.push(message),
                }
            }
            Json(json!({
            "branch": outcome.branch,
            "files": outcome.files,
            "commit": outcome.commit,
            "pagesUrl": outcome.pages_url,
            "settingsUrl": outcome.settings_url,
            "warnings": outcome.warnings,
            }))
            .into_response()
        }
    }
}

/// Pages 를 켜고 실제 주소를 돌려줍니다.
fn pages_for(
    config: &Config,
    root: &std::path::Path,
    token: &deploy::Token,
    branch: &str,
) -> Result<Option<String>, Message> {
    let Some(url) = current_remote(root) else {
        return Ok(None);
    };
    let Some(repo) = deploy::parse_github(&url) else {
        return Ok(None);
    };

    match github::enable_pages(token, &repo.owner, &repo.name, branch) {
        Ok(info) => Ok(info.url.or_else(|| {
            // 방금 켰으면 주소가 아직 안 올 수 있습니다. 규칙대로 만듭니다.
            config
                .deploy
                .cname
                .clone()
                .map(|d| format!("https://{d}/"))
                .or_else(|| Some(repo.pages_url()))
        })),
        Err(err) => Err(Message::new("msg.api.pagesFailed")
            .with("detail", err.message().key)),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 이미지 올리기
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct UploadQuery {
    /// 원본 파일 이름. 확장자를 알아내는 데만 씁니다.
    name: String,
    /// 저장할 이름의 앞부분. `avatar`, `thumb` 처럼 용도를 나타냅니다.
    #[serde(default)]
    kind: Option<String>,
}

/// 이미지를 `assets/` 에 저장하고 설정에 넣을 상대 경로를 돌려줍니다.
///
/// multipart 대신 본문에 바이트를 그대로 받습니다. 의존성이 하나 줄고,
/// 브라우저에서는 `fetch(file)` 한 줄이라 더 간단합니다.
async fn post_upload(
    State(state): State<Shared>,
    Query(query): Query<UploadQuery>,
    body: axum::body::Bytes,
) -> Response {
    const MAX_BYTES: usize = 8 * 1024 * 1024;

    if body.is_empty() {
        return error_json(Message::new("msg.upload.empty"));
    }
    if body.len() > MAX_BYTES {
        return error_json(
            Message::new("msg.upload.tooBig")
                .with("size", format!("{:.1}", body.len() as f64 / 1024.0 / 1024.0)),
        );
    }

    // 확장자는 허용 목록에서만 고릅니다. 원본 이름을 그대로 쓰면 경로를
    // 거슬러 올라가거나 실행 가능한 파일을 심을 여지가 생깁니다.
    let extension = std::path::Path::new(&query.name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());

    let extension = match extension.as_deref() {
        Some("png") => "png",
        Some("jpg") | Some("jpeg") => "jpg",
        Some("gif") => "gif",
        Some("webp") => "webp",
        Some("svg") => "svg",
        other => {
            return error_json(
                Message::new("msg.upload.badFormat")
                    .with("format", other.unwrap_or("?")),
            )
        }
    };

    // 파일 내용이 정말 그 형식인지 봅니다. 확장자만 바꿔 올린 파일이
    // 브라우저에서 깨져 보이는 것을 미리 막습니다.
    if !looks_like_image(&body, extension) {
        return error_json(Message::new("msg.upload.notAnImage").with("format", extension));
    }

    let kind = query.kind.as_deref().unwrap_or("image");
    let kind: String = kind
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '-')
        .take(24)
        .collect();
    let kind = if kind.is_empty() {
        "image".to_string()
    } else {
        kind
    };

    let root = state.lock().unwrap().root.clone();
    let assets = root.join("assets");
    if let Err(err) = std::fs::create_dir_all(&assets) {
        return error_json(Message::new("msg.upload.noAssetsDir").with("detail", err));
    }

    // 같은 이름을 덮어쓰면 브라우저 캐시 때문에 옛 사진이 계속 보입니다.
    // 시각을 붙여 새 파일로 둡니다.
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let filename = format!("{kind}-{stamp}.{extension}");

    if let Err(err) = std::fs::write(assets.join(&filename), &body) {
        return error_json(Message::new("msg.upload.saveFailed").with("detail", err));
    }

    Json(json!({ "path": format!("assets/{filename}") })).into_response()
}

/// 파일 앞부분의 매직 넘버를 봅니다.
fn looks_like_image(bytes: &[u8], extension: &str) -> bool {
    match extension {
        "png" => bytes.starts_with(&[0x89, b'P', b'N', b'G']),
        "jpg" => bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
        "gif" => bytes.starts_with(b"GIF8"),
        "webp" => bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP",
        // SVG 는 텍스트라 매직 넘버가 없습니다. 앞부분에 태그가 있는지만 봅니다.
        "svg" => {
            let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).to_lowercase();
            head.contains("<svg") || head.contains("<?xml")
        }
        _ => false,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 미리보기
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct PreviewQuery {
    lang: Option<String>,
}

/// 디스크에 쓰지 않고 그때그때 렌더링합니다. 저장하지 않은 상태를 보여주는
/// 것이 아니라 **저장된 설정**을 보여줍니다 — 편집기가 저장 후 새로고침합니다.
async fn preview(State(state): State<Shared>, Query(query): Query<PreviewQuery>) -> Response {
    let (path, root) = {
        let state = state.lock().unwrap();
        (state.config_path.clone(), state.root.clone())
    };

    let source = match std::fs::read_to_string(&path) {
        Ok(source) => source,
        Err(err) => return error_html(&format!("읽기 실패: {err}")),
    };
    let config: Config = match toml::from_str(&source) {
        Ok(config) => config,
        Err(err) => return error_html(&format!("파싱 실패:\n{err}")),
    };

    let languages = config.languages();
    let default_lang = config.default_language().to_string();
    let lang = query
        .lang
        .filter(|l| languages.contains(l))
        .unwrap_or_else(|| default_lang.clone());

    let strings = match Strings::load(&lang, &root) {
        Ok(strings) => strings,
        Err(err) => return error_html(&err),
    };

    // 미리보기는 모든 언어가 같은 경로에 있으므로 접두사가 필요 없습니다.
    // 언어 전환은 편집기 쪽 탭이 담당합니다.
    let entries: Vec<(String, String, String)> = languages
        .iter()
        .map(|l| {
            (
                l.clone(),
                i18n::language_name(l, &root),
                format!("?lang={l}"),
            )
        })
        .collect();

    let ctx = Ctx {
        config: &config,
        lang: &lang,
        fallback: &default_lang,
        strings: &strings,
        languages: &entries,
        prefix: "",
    };

    Html(render::page(&ctx)).into_response()
}

/// 미리보기에서 참조하는 프로젝트 파일(아바타, 썸네일 등).
async fn preview_asset(
    State(state): State<Shared>,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> Response {
    let root = state.lock().unwrap().root.clone();

    // 프로젝트 밖으로 나가는 경로를 막습니다.
    if path.contains("..") {
        return (StatusCode::FORBIDDEN, "허용되지 않는 경로").into_response();
    }

    let full = root.join(&path);
    match std::fs::read(&full) {
        Err(_) => (StatusCode::NOT_FOUND, "파일 없음").into_response(),
        Ok(bytes) => {
            let mime = match full.extension().and_then(|e| e.to_str()) {
                Some("png") => "image/png",
                Some("jpg") | Some("jpeg") => "image/jpeg",
                Some("gif") => "image/gif",
                Some("webp") => "image/webp",
                Some("svg") => "image/svg+xml",
                Some("ico") => "image/x-icon",
                _ => "application/octet-stream",
            };
            ([(header::CONTENT_TYPE, mime)], bytes).into_response()
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 응답 도우미
// ─────────────────────────────────────────────────────────────────────────────

fn css(body: &'static str) -> Response {
    ([(header::CONTENT_TYPE, "text/css; charset=utf-8")], body).into_response()
}

fn js(body: &'static str) -> Response {
    (
        [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
        body,
    )
        .into_response()
}

/// 오류 응답.
///
/// 문장이 아니라 키와 인자를 보냅니다. 편집기가 자기 화면 언어로 조립합니다.
fn error_json(message: Message) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({ "error": { "key": message.key, "args": message.args } })),
    )
        .into_response()
}

fn error_html(message: &str) -> Response {
    let markup = maud::html! {
        (maud::DOCTYPE)
        html lang="ko" {
            head { meta charset="utf-8"; }
            body style="font: 14px/1.6 system-ui; padding: 24px; color: #b3261e;" {
                h1 style="font-size: 16px;" { "미리보기를 만들 수 없습니다" }
                pre style="white-space: pre-wrap;" { (message) }
            }
        }
    };
    Html(markup.into_string()).into_response()
}

#[cfg(test)]
mod open_tests {
    use super::is_openable;

    /// 셸을 안 타므로 특수문자 자체가 명령이 되지는 않지만, 주소에 공백이나
    /// 제어문자가 섞여 들어오는 것은 정상이 아닙니다.
    #[test]
    fn rejects_anything_that_is_not_a_plain_web_address() {
        assert!(is_openable("https://github.com/settings/tokens/new?scopes=repo"));
        assert!(is_openable("http://127.0.0.1:4180/"));

        // 스킴이 아닌 것
        assert!(!is_openable("file:///C:/Windows/System32/calc.exe"));
        assert!(!is_openable("javascript:alert(1)"));
        assert!(!is_openable("ftp://example.com"));

        // 조작 흔적
        assert!(!is_openable("https://example.com/ & calc"));
        assert!(!is_openable("https://example.com/\ncalc"));
        assert!(!is_openable("https://example.com/\tcalc"));
        assert!(!is_openable("https://example.com/\u{0}calc"));
    }

    /// cmd 를 거치면 명령 구분자가 되는 문자들. 셸을 안 타므로 그대로
    /// 브라우저에 넘어가야 합니다 — 정상적인 질의 문자열이기 때문입니다.
    #[test]
    fn keeps_legitimate_query_strings() {
        assert!(is_openable("https://example.com/?a=1&b=2"));
        assert!(is_openable("https://example.com/?x=100%25"));
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;

    /// 편집기는 Config 를 JSON 으로 주고받습니다. 한 바퀴 돌아 같은 값이
    /// 나오지 않으면 저장할 때마다 설정이 조금씩 망가집니다.
    #[test]
    fn config_round_trips_through_json() {
        let source = std::fs::read_to_string("profile.toml").expect("profile.toml");
        let original: Config = toml::from_str(&source).expect("파싱");

        let json = serde_json::to_value(&original).expect("직렬화");
        let restored: Config = serde_json::from_value(json.clone()).expect("역직렬화");

        assert_eq!(
            serde_json::to_value(&restored).unwrap(),
            json,
            "JSON 왕복에서 값이 달라졌습니다"
        );
    }
}

#[cfg(test)]
mod editor_tests {
    use super::*;
    use std::io::{Read, Write};

    /// 의존성 없이 GET 한 번. 테스트용이라 헤더 처리는 최소한만 합니다.
    fn get(url: &str, path: &str) -> String {
        let authority = url
            .trim_start_matches("http://")
            .trim_end_matches('/')
            .to_string();

        let mut stream = std::net::TcpStream::connect(&authority).expect("연결");
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n\r\n"
        )
        .expect("요청");

        let mut body = String::new();
        stream.read_to_string(&mut body).expect("응답");
        body
    }

    /// Tauri 앱이 기대는 동작: **서버를 다시 띄우지 않고** 다른 명함으로
    /// 갈아탑니다. 이게 깨지면 폴더를 바꿔도 옛 명함이 계속 보입니다.
    #[test]
    fn switching_projects_does_not_need_a_restart() {
        let base = std::env::temp_dir().join("profileit-editor-switch");
        let first = base.join("first");
        let second = base.join("second");

        for dir in [&first, &second] {
            std::fs::create_dir_all(dir).expect("폴더");
            let config = dir.join("profile.toml");
            let _ = std::fs::remove_file(&config);
            crate::init::init(&config).expect("init");
        }

        // 이름을 서로 다르게 바꿔 어느 쪽이 보이는지 구분합니다.
        for (dir, name) in [(&first, "첫째명함"), (&second, "둘째명함")] {
            let config = dir.join("profile.toml");
            let source = std::fs::read_to_string(&config).expect("읽기");
            std::fs::write(&config, source.replace("name = \"이름\"", &format!("name = \"{name}\"")))
                .expect("쓰기");
        }

        let editor = spawn(&first.join("profile.toml")).expect("서버");
        assert!(get(editor.url(), "/api/config").contains("첫째명함"));

        editor.open(&second.join("profile.toml"));

        // 같은 서버, 같은 주소 — 내용만 바뀝니다.
        let after = get(editor.url(), "/api/config");
        assert!(after.contains("둘째명함"), "폴더 전환이 반영되지 않았습니다");
        assert!(!after.contains("첫째명함"));
        assert_eq!(editor.config_path(), second.join("profile.toml"));
    }
}
