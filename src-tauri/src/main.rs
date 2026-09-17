//! ProfileIT desktop editor.
//!
//! A thin shell that opens a single window with the editing UI. All editing,
//! validation, and rendering lives in the `profileit` library — this crate
//! only owns the window and the menu.
//!
//! **There is no child process.** The edit server runs inside this same
//! process, so it shuts down with the app (no orphaned processes left
//! behind), and switching to a different card folder doesn't require
//! restarting the server.

// Keep the console window from popping up alongside release builds.
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

/// Language used by the shell (menu, dialogs).
///
/// Uses `PROFILEIT_LANG` if set, otherwise falls back to this card's default
/// language. This is independent of the language picker inside the editor —
/// the menu is drawn once when the window is created and then owned by the
/// OS, so changing the language in the editor can't redraw it. It catches up
/// the next time the app is opened.
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

    // Start the server first — we need its address before we can point the window at it.
    let editor = match serve::spawn(&start) {
        Ok(editor) => editor,
        Err(err) => {
            // Dying silently without ever showing a window would leave no clue what went wrong.
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
        // The window can't navigate outside the editor. If a blog link in the
        // preview took the window to that site, there'd be no good way back.
        // Outside addresses are sent to the default browser instead.
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
// Starting folder
// ─────────────────────────────────────────────────────────────────────────────

/// File where we remember the last folder that was opened.
fn settings_path(app: &AppHandle) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("last.txt"))
}

fn remember(app: &AppHandle, config_path: &Path) {
    let Some(path) = settings_path(app) else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // If we fail to remember it, this run still works fine.
    let _ = std::fs::write(path, config_path.to_string_lossy().as_bytes());
}

/// Decides where to open. Order: last folder → current working directory → manual pick.
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

    // Nothing found, so let the user pick. If they cancel here there's nothing to open, so we exit.
    match pick_project(app) {
        Some(path) => path,
        None => std::process::exit(0),
    }
}

/// Lets the user pick a folder and, if it has no `profile.toml`, asks whether to create one.
fn pick_project(app: &AppHandle) -> Option<PathBuf> {
    let folder = app.dialog().file().blocking_pick_folder()?;
    let folder: PathBuf = folder.into_path().ok()?;
    let config_path = folder.join(CONFIG_NAME);

    if config_path.exists() {
        return Some(config_path);
    }

    // There's no card to read yet, so it's `PROFILEIT_LANG` or Korean.
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

    // We never overwrite an existing file — `init` guards against that.
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

/// Sends an outside address to the default browser.
///
/// Uses `tauri-plugin-opener`. Calling `cmd /C start` directly would let cmd
/// reinterpret the command line under its own rules, so `&`, `|`, and `%` in
/// the address could act as command separators or environment-variable
/// expansions. The plugin calls the OS's shell-execute API directly and
/// skips that reinterpretation step.
fn open_outside(app: &AppHandle, url: &str) {
    // A well-formed address has no whitespace or control characters. If it does, it's been tampered with.
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
// Menu
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

/// Switches to a different card.
///
/// Doesn't restart the server — just swaps the state and reloads the window.
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
    // The menu is already owned by the OS, so it can't be redrawn in the new
    // language — that catches up next time the app opens. The window title
    // and content update right away, though.
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
