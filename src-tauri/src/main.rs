//! ProfileIT 데스크톱 편집기.
//!
//! 창 하나에 편집 UI 를 띄우는 얇은 껍데기입니다. 편집·검증·렌더링은 전부
//! `profileit` 라이브러리에 있고, 여기서는 창과 메뉴만 담당합니다.
//!
//! **자식 프로세스가 없습니다.** 편집 서버가 같은 프로세스 안에서 도는 덕분에,
//! 앱이 끝나면 서버도 같이 끝나고(고아 프로세스가 남지 않습니다) 다른 명함
//! 폴더로 옮겨갈 때도 서버를 다시 띄우지 않습니다.

// 릴리스 빌드에서 콘솔 창이 함께 뜨지 않게 합니다.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::{Path, PathBuf};

use profileit::i18n::Strings;
use profileit::message::Message;
use profileit::serve::{self, Editor};
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{App, AppHandle, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_opener::OpenerExt;

const CONFIG_NAME: &str = "profile.toml";

/// 껍데기(메뉴·대화상자)에 쓸 언어.
///
/// `PROFILEIT_LANG` 이 있으면 그것을, 없으면 이 명함의 기본 언어를 씁니다.
/// 편집기 안의 화면 언어 선택과는 따로 움직입니다 — 메뉴는 창을 만들 때 한 번
/// 그려지고 OS 가 들고 있어서, 편집기에서 언어를 바꿔도 다시 그릴 수 없습니다.
/// 다음에 앱을 열면 맞춰집니다.
fn app_strings(config_path: &Path) -> Strings {
    let root = profileit::project_root(config_path);
    let chosen = std::env::var("PROFILEIT_LANG").ok().unwrap_or_else(|| {
        profileit::load(config_path)
            .map(|(config, _)| config.default_language().to_string())
            .unwrap_or_else(|_| "ko".to_string())
    });

    Strings::load(&chosen, root)
        .or_else(|_| Strings::load("ko", root))
        .unwrap_or_default()
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(setup)
        .on_menu_event(on_menu_event)
        .run(tauri::generate_context!())
        .expect("ProfileIT could not start");
}

fn setup(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle().clone();
    let start = first_project(&handle);
    let strings = app_strings(&start);

    // 서버를 먼저 띄웁니다. 주소를 알아야 창을 그리로 보낼 수 있습니다.
    let editor = match serve::spawn(&start) {
        Ok(editor) => editor,
        Err(err) => {
            // 창도 못 띄운 채 조용히 죽으면 무엇이 잘못됐는지 알 길이 없습니다.
            app.dialog()
                .message(
                    Message::new("app.serverFailed")
                        .with("detail", err)
                        .render(&strings),
                )
                .kind(MessageDialogKind::Error)
                .title("ProfileIT")
                .blocking_show();
            std::process::exit(1);
        }
    };

    let base = editor.url().to_string();
    let url = base.parse()?;
    app.manage(editor);

    let outside = base.clone();
    let nav_handle = handle.clone();
    let window = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
        .title(title_for(&start))
        .inner_size(1280.0, 860.0)
        .min_inner_size(760.0, 560.0)
        // 편집기 밖으로는 못 나갑니다. 미리보기 안의 블로그 링크를 눌렀을 때
        // 창이 그 사이트로 넘어가 버리면 돌아올 방법이 마땅치 않습니다.
        // 바깥 주소는 기본 브라우저로 보냅니다.
        .on_navigation(move |target| {
            let target = target.to_string();
            if target.starts_with(&outside) || target.starts_with("about:") {
                return true;
            }
            open_outside(&nav_handle, &target);
            false
        })
        .build()?;

    window.set_menu(build_menu(&handle, &strings)?)?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// 시작 폴더
// ─────────────────────────────────────────────────────────────────────────────

/// 마지막으로 열었던 폴더를 기억해 둘 파일.
fn settings_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("last.txt"))
}

fn remember(app: &AppHandle, config_path: &Path) {
    let Some(path) = settings_path(app) else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // 기억해 두지 못해도 이번 실행은 문제없이 굴러갑니다.
    let _ = std::fs::write(path, config_path.to_string_lossy().as_bytes());
}

/// 열 곳을 정합니다. 마지막 폴더 → 현재 작업 폴더 → 직접 고르기 순입니다.
fn first_project(app: &AppHandle) -> PathBuf {
    let remembered = settings_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(PathBuf::from)
        .filter(|p| p.exists());

    if let Some(path) = remembered {
        return path;
    }

    let here = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(CONFIG_NAME);
    if here.exists() {
        return here;
    }

    // 아무것도 없으면 고르게 합니다. 여기서 취소하면 열 것이 없으므로 끝냅니다.
    match pick_project(app) {
        Some(path) => path,
        None => std::process::exit(0),
    }
}

/// 폴더를 고르고, `profile.toml` 이 없으면 만들지 물어봅니다.
fn pick_project(app: &AppHandle) -> Option<PathBuf> {
    let folder = app.dialog().file().blocking_pick_folder()?;
    let folder: PathBuf = folder.into_path().ok()?;
    let config_path = folder.join(CONFIG_NAME);

    if config_path.exists() {
        return Some(config_path);
    }

    // 읽을 명함이 아직 없으니 `PROFILEIT_LANG` 아니면 한국어입니다.
    let strings = app_strings(&config_path);
    let create = app
        .dialog()
        .message(
            Message::new("app.create.question")
                .with("folder", folder.display())
                .with("file", CONFIG_NAME)
                .render(&strings),
        )
        .title(strings.get("app.create.title"))
        .buttons(MessageDialogButtons::OkCancelCustom(
            strings.get("app.create.ok").to_string(),
            strings.get("app.create.cancel").to_string(),
        ))
        .blocking_show();

    if !create {
        return None;
    }

    // 있는 파일을 덮어쓰는 일은 없습니다 — init 쪽에서 막습니다.
    match profileit::init::init(&config_path) {
        Ok(()) => Some(config_path),
        Err(err) => {
            app.dialog()
                .message(
                    Message::new("app.createFailed")
                        .with("detail", err)
                        .render(&strings),
                )
                .kind(MessageDialogKind::Error)
                .blocking_show();
            None
        }
    }
}

/// 바깥 주소를 기본 브라우저로 보냅니다.
///
/// `tauri-plugin-opener` 를 씁니다. 직접 `cmd /C start` 를 부르면 cmd 가
/// 명령줄을 자기 규칙으로 다시 해석해서, 주소 안의 `&`·`|`·`%` 가 명령
/// 구분자나 환경 변수 확장으로 동작합니다. 플러그인은 OS 의 셸 실행 API 를
/// 직접 불러 그 해석 단계를 거치지 않습니다.
fn open_outside(app: &AppHandle, url: &str) {
    // 정상적인 주소에는 공백이나 제어문자가 없습니다. 있다면 조작된 값입니다.
    let clean = !url.chars().any(|c| c.is_control() || c.is_whitespace());
    if !clean || !(url.starts_with("http://") || url.starts_with("https://")) {
        eprintln!("that address cannot be opened");
        return;
    }

    if let Err(err) = app.opener().open_url(url, None::<&str>) {
        eprintln!("could not open the browser: {err}");
    }
}

fn title_for(config_path: &Path) -> String {
    let name = config_path
        .parent()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string());

    match name {
        Some(name) => format!("ProfileIT — {name}"),
        None => "ProfileIT".to_string(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 메뉴
// ─────────────────────────────────────────────────────────────────────────────

fn build_menu(app: &AppHandle, strings: &Strings) -> tauri::Result<Menu<tauri::Wry>> {
    let file = Submenu::with_items(
        app,
        strings.get("app.menu.file"),
        true,
        &[
            &MenuItem::with_id(
                app,
                "open",
                strings.get("app.menu.open"),
                true,
                Some("CmdOrCtrl+O"),
            )?,
            &MenuItem::with_id(
                app,
                "open-dist",
                strings.get("app.menu.openDist"),
                true,
                None::<&str>,
            )?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::quit(app, Some(strings.get("app.menu.quit")))?,
        ],
    )?;

    let view = Submenu::with_items(
        app,
        strings.get("app.menu.view"),
        true,
        &[
            &MenuItem::with_id(
                app,
                "reload",
                strings.get("app.menu.reload"),
                true,
                Some("CmdOrCtrl+R"),
            )?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(
                app,
                "docs",
                strings.get("app.menu.docs"),
                true,
                None::<&str>,
            )?,
        ],
    )?;

    Menu::with_items(app, &[&file, &view])
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };

    match event.id().as_ref() {
        "open" => open_another(app, &window),
        "open-dist" => open_dist(app),
        "reload" => {
            let _ = window.eval("location.reload()");
        }
        "docs" => {
            let docs = Path::new("docs").join("schema.md");
            if docs.exists() {
                let _ = app.opener().open_path(docs.to_string_lossy(), None::<&str>);
            }
        }
        _ => {}
    }
}

/// 다른 명함으로 갈아탑니다.
///
/// 서버를 다시 띄우지 않습니다 — 상태만 바꾸고 창을 새로고침하면 끝입니다.
fn open_another(app: &AppHandle, window: &WebviewWindow) {
    let Some(config_path) = pick_project(app) else {
        return;
    };
    let Some(editor) = app.try_state::<Editor>() else {
        return;
    };

    editor.open(&config_path);
    remember(app, &config_path);

    let _ = window.set_title(&title_for(&config_path));
    // 메뉴는 이미 OS 가 들고 있어서 새 언어로 다시 그릴 수 없습니다. 다음에 열 때
    // 맞춰집니다 — 창 제목과 내용은 지금 바로 바뀝니다.
    let _ = window.eval("location.reload()");
}

fn open_dist(app: &AppHandle) {
    let Some(editor) = app.try_state::<Editor>() else {
        return;
    };

    let dist = editor
        .config_path()
        .parent()
        .unwrap_or(Path::new("."))
        .join("dist");

    if !dist.exists() {
        app.dialog()
            .message(app_strings(&editor.config_path()).get("app.noDist"))
            .title("ProfileIT")
            .blocking_show();
        return;
    }

    let _ = app.opener().open_path(dist.to_string_lossy(), None::<&str>);
}
