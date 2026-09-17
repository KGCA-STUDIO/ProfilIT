//! The local editing UI.
//!
//! A small axum server that only binds to `127.0.0.1`. The left side is the
//! form; the right side is an iframe holding the **actual renderer output**,
//! so the preview can never drift from the real result. A separate
//! editor-only rendering path would have gradually diverged from it.
//!
//! There's no authentication. It only binds to loopback, so other devices
//! can't reach it.

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

/// A handle to the running editor server.
///
/// Lets the desktop app switch to a different card folder **without
/// restarting the server**. That's only possible because it's in the same
/// process — a child process would have to be killed, respawned, and its
/// port waited on all over again.
#[derive(Clone)]
pub struct Editor {
    url: String,
    state: Shared,
}

impl Editor {
    pub fn url(&self) -> &str {
        &self.url
    }

    /// The config file currently open.
    pub fn config_path(&self) -> PathBuf {
        self.state.lock().unwrap().config_path.clone()
    }

    /// Switches to a different card. The caller is responsible for refreshing the window.
    pub fn open(&self, config_path: &Path) {
        let root = crate::project_root(config_path).to_path_buf();

        let mut state = self.state.lock().unwrap();
        state.config_path = config_path.to_path_buf();
        state.root = root;
    }
}

/// Launched from the terminal. Doesn't return until Ctrl+C.
pub fn run(config_path: &Path) -> std::io::Result<()> {
    let app = router(new_state(config_path));

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    // Language for terminal output. Separate from the editor's UI language —
    // these lines print before the editor is even up, before we could know
    // what language the page will end up choosing.
    let strings = terminal_strings(config_path);
    // Build the closing message ahead of time. The shutdown future can outlive
    // `strings`, so it can't carry a reference into it.
    let closed = strings.get("msg.serve.closed").to_string();

    runtime.block_on(async move {
        let (listener, addr) = bind().await?;
        let url = format!("http://{addr}/");

        // A line the desktop app parses. The human-readable message can be
        // translated, so we keep a separate marker for machines to look for.
        println!("PROFILEIT_LISTENING {url}");
        println!(
            "{}",
            Message::new("msg.serve.opened")
                .with("url", &url)
                .render(&strings)
        );
        println!("{}", strings.get("msg.serve.stop"));

        // Inside the desktop app there's already a window, so we don't open a browser too.
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

/// Spawns in-process and returns the address.
///
/// Used by the desktop app (`src-tauri/`). Since both are Rust, none of the
/// usual dance — spawning a child process, passing the port, waiting for it
/// to come up — is needed at all. And since the server dies with the app,
/// there's no orphan process left behind either.
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

                // The window can't open until it knows the address.
                if tx.send(Ok(format!("http://{addr}/"))).is_err() {
                    return; // Whoever was waiting is gone, so there's no reason to keep serving.
                }
                let _ = axum::serve(listener, app).await;
            });
        })?;

    let url = rx.recv().map_err(|_| {
        std::io::Error::new(std::io::ErrorKind::Other, "편집 서버 스레드가 멈췄습니다")
    })??;

    Ok(Editor { url, state })
}

/// Language for terminal output. Uses `PROFILEIT_LANG` if set, otherwise this
/// card's default language — the same rule as the CLI (`src/main.rs`).
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

/// The router, shared by `run` and `spawn`.
///
/// Only binds to loopback. Binding to 0.0.0.0 would let other devices on the
/// same network edit the config, which isn't okay given there's no authentication.
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

/// If the default port is taken, tries a few more. Running two editors at
/// once isn't unusual, and we don't want to make people pick a port manually
/// every time.
///
/// If `PROFILEIT_PORT` is set, only that port is used. The desktop app claims
/// a free port first and hands it over, so falling back to a different port
/// would leave the window pointed at the wrong place.
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

/// Opens the URL in the default browser.
///
/// **Never goes through a shell.** Using `cmd /C start` on Windows would have
/// cmd reinterpret the command line by its own rules, so `&`, `|`, `%` inside
/// the URL would act as command separators or environment-variable
/// expansion. A single URL like `https://example.com/?a=1&calc` could launch
/// an unrelated program.
///
/// `rundll32 url.dll,FileProtocolHandler` hands off straight to the
/// registered default browser without touching a shell. On Unix, `Command`
/// never goes through a shell to begin with, so `open`/`xdg-open` can be
/// called directly.
fn open_browser(url: &str) -> bool {
    if !is_openable(url) {
        // The message is up to the caller to render in its own language. We
        // only report whether it opened.
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

    // Not fatal even if the browser doesn't open — the URL was already printed.
    result.is_ok()
}

/// Whether this URL is safe to hand off.
///
/// Since we never go through a shell, special characters aren't dangerous by
/// themselves — but a legitimate URL never contains whitespace or control
/// characters. If it does, the value has been tampered with somewhere.
fn is_openable(url: &str) -> bool {
    let http = url.starts_with("http://") || url.starts_with("https://");
    let clean = !url.chars().any(|c| c.is_control() || c.is_whitespace());
    http && clean
}

// ─────────────────────────────────────────────────────────────────────────────
// API
// ─────────────────────────────────────────────────────────────────────────────

/// A diagnostic sent to the editor.
///
/// Sent as a key and arguments rather than a finished sentence. The editor
/// assembles it in its own UI language, so it reads correctly regardless of
/// what language the server happens to run in.
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

/// The lists the editor uses to fill its dropdowns.
///
/// Why the server sends these instead of hardcoding them in JS: it's easy to
/// forget to update the editor's JS when a font preset or language gets added.
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

    // Send the editor's UI strings for every language up front, so switching
    // the display language can happen instantly without asking the server again.
    //
    // `msg.` is included too. Errors and warnings from the server are keys,
    // not finished sentences, so the editor needs its own lookup table to
    // render them in the UI language.
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

    // Check the shape first. If this fails, the file is left untouched.
    let config: Config = match serde_json::from_value(body.clone()) {
        Ok(config) => config,
        Err(err) => {
            return error_json(Message::new("msg.api.badShape").with("detail", err))
        }
    };

    let diagnostics = validate::validate(&config, &root);
    if validate::has_errors(&diagnostics) {
        // Don't save if there are errors — better to fix it in the editor than
        // to let a broken config land on disk.
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

/// The editor's "Build Site" button. Builds `dist/` from the saved config.
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

/// Opens a URL in the default browser.
///
/// The desktop app's webview blocks `window.open`. Even if it didn't, there'd
/// be no good way back once the editor window navigated to an external site.
/// So the server opens it instead — the same code path works whether we're
/// inside the app or a regular browser.
async fn post_open(Json(body): Json<OpenBody>) -> Response {
    let url = body.url.trim();

    // The decision is made in exactly one place, `is_openable`. A separate
    // check here could drift out of sync and report success for a URL that
    // never actually opened.
    if !is_openable(url) {
        return error_json(Message::new("msg.api.notOpenable"));
    }

    open_browser(url);
    Json(json!({ "opened": true })).into_response()
}

// ─────────────────────────────────────────────────────────────────────────────
// GitHub connection
// ─────────────────────────────────────────────────────────────────────────────

/// Connection status. Returns everything the editor needs to render the page in one shot.
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

    // The token might have expired or been revoked. Check that before fetching the repo list.
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

/// The remote the current project points at. Shows which repository is selected.
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

    // Try it before storing it. A bad token saved to the store would look
    // connected on the next launch, then fail as soon as something real is attempted.
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
    /// Set when an existing repository was picked.
    #[serde(default)]
    clone_url: Option<String>,
    /// Set when creating a new one.
    #[serde(default)]
    create: Option<String>,
    #[serde(default)]
    private: bool,
}

/// Picks or creates a repository, and points this folder's remote at it.
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

/// The editor's "Deploy to GitHub" button.
///
/// Always rebuilds before pushing — pushing a dist that's missing recent
/// edits is the most common mistake here.
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
            // Enable Pages after pushing. Without this, the push succeeds but
            // the URL stays a 404, and it's hard to tell what's missing.
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

/// Enables Pages and returns the actual URL.
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
            // If Pages was just enabled, the URL might not be back yet. Build it by convention.
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
// Image uploads
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct UploadQuery {
    /// The original filename. Only used to work out the extension.
    name: String,
    /// The prefix of the saved filename, indicating its purpose — `avatar`, `thumb`, etc.
    #[serde(default)]
    kind: Option<String>,
}

/// Saves an image to `assets/` and returns the relative path to put in the config.
///
/// Takes raw bytes in the body instead of multipart. One less dependency, and
/// simpler on the browser side too — just a `fetch(file)` call.
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

    // The extension is only chosen from an allowlist. Using the original name
    // as-is would open the door to path traversal or planting an executable file.
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

    // Check that the file content actually matches the claimed format. This
    // heads off a file uploaded with just its extension swapped, which would
    // otherwise show up broken in the browser.
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

    // Overwriting the same name would keep showing the old photo due to
    // browser caching, so append a timestamp to make it a new file.
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

/// Checks the magic number at the start of the file.
fn looks_like_image(bytes: &[u8], extension: &str) -> bool {
    match extension {
        "png" => bytes.starts_with(&[0x89, b'P', b'N', b'G']),
        "jpg" => bytes.starts_with(&[0xFF, 0xD8, 0xFF]),
        "gif" => bytes.starts_with(b"GIF8"),
        "webp" => bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP",
        // SVG is text, so it has no magic number — just check for a tag near the start.
        "svg" => {
            let head = String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).to_lowercase();
            head.contains("<svg") || head.contains("<?xml")
        }
        _ => false,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Preview
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct PreviewQuery {
    lang: Option<String>,
}

/// Renders on the fly instead of writing to disk. This shows the **saved
/// config**, not unsaved edits — the editor refreshes it after saving.
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

    // Every language shares the same path in the preview, so no prefix is
    // needed. Switching languages is handled by tabs on the editor side.
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

/// Project files the preview references (avatar, thumbnails, etc.).
async fn preview_asset(
    State(state): State<Shared>,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> Response {
    let root = state.lock().unwrap().root.clone();

    // Block any path that escapes the project.
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
// Response helpers
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

/// An error response.
///
/// Sent as a key and arguments rather than a finished sentence; the editor assembles it in its own UI language.
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

    /// Since we never go through a shell, special characters aren't commands
    /// by themselves, but whitespace or control characters showing up in a
    /// URL is never normal.
    #[test]
    fn rejects_anything_that_is_not_a_plain_web_address() {
        assert!(is_openable("https://github.com/settings/tokens/new?scopes=repo"));
        assert!(is_openable("http://127.0.0.1:4180/"));

        // Not a web scheme
        assert!(!is_openable("file:///C:/Windows/System32/calc.exe"));
        assert!(!is_openable("javascript:alert(1)"));
        assert!(!is_openable("ftp://example.com"));

        // Signs of tampering
        assert!(!is_openable("https://example.com/ & calc"));
        assert!(!is_openable("https://example.com/\ncalc"));
        assert!(!is_openable("https://example.com/\tcalc"));
        assert!(!is_openable("https://example.com/\u{0}calc"));
    }

    /// Characters that would act as command separators through cmd. We never
    /// go through a shell, so they should pass straight through to the
    /// browser — they're a legitimate part of a query string.
    #[test]
    fn keeps_legitimate_query_strings() {
        assert!(is_openable("https://example.com/?a=1&b=2"));
        assert!(is_openable("https://example.com/?x=100%25"));
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Config;

    /// The editor exchanges Config as JSON. If a round trip doesn't come back
    /// to the same value, the config would drift a little more corrupt with each save.
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

    /// A single GET with no dependencies. Header handling is minimal since this is test-only.
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

    /// The behavior the Tauri app relies on: switching to a different card
    /// **without restarting the server**. If this breaks, changing folders
    /// would keep showing the old card.
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

        // Give each a distinct name so we can tell which one is showing.
        for (dir, name) in [(&first, "첫째명함"), (&second, "둘째명함")] {
            let config = dir.join("profile.toml");
            let source = std::fs::read_to_string(&config).expect("읽기");
            std::fs::write(&config, source.replace("name = \"이름\"", &format!("name = \"{name}\"")))
                .expect("쓰기");
        }

        let editor = spawn(&first.join("profile.toml")).expect("서버");
        assert!(get(editor.url(), "/api/config").contains("첫째명함"));

        editor.open(&second.join("profile.toml"));

        // Same server, same address — only the content changes.
        let after = get(editor.url(), "/api/config");
        assert!(after.contains("둘째명함"), "폴더 전환이 반영되지 않았습니다");
        assert!(!after.contains("첫째명함"));
        assert_eq!(editor.config_path(), second.join("profile.toml"));
    }
}
