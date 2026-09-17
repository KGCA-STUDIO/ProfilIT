//! profileit CLI.
//!
//! All the real work lives in the library (`src/lib.rs`). This file just
//! picks a command and prints the result in a human-readable way.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use profileit::i18n::Strings;
use profileit::message::Message;
use profileit::validate::{Diagnostic, Severity};
use profileit::{build, deploy, init, load, project_root, LoadError};

const DEFAULT_CONFIG: &str = "profile.toml";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);

    let command = args.next().unwrap_or_else(|| "build".to_string());
    let path = PathBuf::from(args.next().unwrap_or_else(|| DEFAULT_CONFIG.to_string()));

    match command.as_str() {
        "check" => run_check(&path),
        "build" => run_build(&path),
        "init" => run_init(&path),
        "deploy" => run_deploy(&path),
        #[cfg(feature = "editor")]
        "edit" => run_edit(&path),
        "-h" | "--help" | "help" => {
            print_usage(&cli_strings(&path, None));
            ExitCode::SUCCESS
        }
        other => {
            let strings = cli_strings(&path, None);
            say(
                Message::new("cli.unknownCommand").with("command", other),
                &strings,
            );
            eprintln!();
            print_usage(&strings);
            ExitCode::FAILURE
        }
    }
}

fn print_usage(strings: &Strings) {
    eprintln!("{}", strings.get("cli.usage.header"));
    eprintln!();
    eprintln!("{}", strings.get("cli.usage.commands"));
    eprintln!("  build    {}", strings.get("cli.usage.build"));
    eprintln!("  check    {}", strings.get("cli.usage.check"));
    eprintln!("  init     {}", strings.get("cli.usage.init"));
    eprintln!("  deploy   {}", strings.get("cli.usage.deploy"));
    #[cfg(feature = "editor")]
    eprintln!("  edit     {}", strings.get("cli.usage.edit"));
    eprintln!();
    say(
        Message::new("cli.usage.default").with("file", DEFAULT_CONFIG),
        strings,
    );
}

/// Renders and prints a single line in the display language.
fn say(message: Message, strings: &Strings) {
    eprintln!("{}", message.render(strings));
}

// ─────────────────────────────────────────────────────────────────────────────
// Commands
// ─────────────────────────────────────────────────────────────────────────────

fn run_check(path: &Path) -> ExitCode {
    match load(path) {
        Err(err) => report_load_error(path, err),
        Ok((config, diagnostics)) => {
            let strings = cli_strings(path, Some(&config));
            report(&diagnostics, &strings);
            println!(
                "{}",
                Message::new("cli.checkOk")
                    .with("sections", config.visible_sections().count())
                    .with("socials", config.socials.len())
                    .with("warnings", diagnostics.len())
                    .render(&strings)
            );
            ExitCode::SUCCESS
        }
    }
}

fn run_build(path: &Path) -> ExitCode {
    let (config, diagnostics) = match load(path) {
        Err(err) => return report_load_error(path, err),
        Ok(loaded) => loaded,
    };
    let strings = cli_strings(path, Some(&config));
    report(&diagnostics, &strings);

    let root = project_root(path);
    let dist = build::default_dist(path);

    match build::build(&config, root, &dist) {
        Err(err) => {
            say(Message::new("cli.buildFailed").with("detail", err), &strings);
            ExitCode::FAILURE
        }
        Ok(output) => {
            println!(
                "{}",
                Message::new("cli.buildOk")
                    .with("dist", dist.display())
                    .with("languages", output.languages.join(", "))
                    .with("kb", format!("{:.1}", output.html_bytes as f64 / 1024.0))
                    .with("assets", output.assets_copied)
                    .with("warnings", diagnostics.len())
                    .render(&strings)
            );
            ExitCode::SUCCESS
        }
    }
}

/// Deploy. **Builds first** — uploading a stale dist that doesn't reflect
/// recent edits is the most common mistake, so we always rebuild what's about
/// to be uploaded right here.
fn run_deploy(path: &Path) -> ExitCode {
    let (config, diagnostics) = match load(path) {
        Err(err) => return report_load_error(path, err),
        Ok(loaded) => loaded,
    };
    let strings = cli_strings(path, Some(&config));
    report(&diagnostics, &strings);

    let root = project_root(path);
    let dist = build::default_dist(path);

    if let Err(err) = build::build(&config, root, &dist) {
        say(Message::new("cli.buildFailed").with("detail", err), &strings);
        return ExitCode::FAILURE;
    }

    // Use a connected token if there is one; otherwise fall back to git's own credentials.
    let token = stored_token();
    match deploy::publish(&config, root, &dist, token.as_ref()) {
        Err(err) => {
            say(
                Message::new("cli.deployFailed").with("detail", err.message().render(&strings)),
                &strings,
            );
            ExitCode::FAILURE
        }
        Ok(outcome) => {
            println!(
                "{}",
                Message::new("cli.deployOk")
                    .with("branch", &outcome.branch)
                    .with("files", outcome.files)
                    .with("commit", &outcome.commit[..outcome.commit.len().min(8)])
                    .render(&strings)
            );
            if let Some(url) = &outcome.pages_url {
                println!(
                    "{}",
                    Message::new("cli.deployUrl")
                        .with("url", url)
                        .render(&strings)
                );
            }
            if let Some(url) = &outcome.settings_url {
                println!(
                    "{}",
                    Message::new("cli.deploySettings")
                        .with("url", url)
                        .render(&strings)
                );
            }
            for warning in &outcome.warnings {
                eprintln!(
                    "[{}] {}",
                    strings.get("cli.warning"),
                    warning.render(&strings)
                );
            }
            ExitCode::SUCCESS
        }
    }
}

/// The stored GitHub token.
///
/// Always `None` when built without the editor feature, since there's no way
/// to read the credential store in that case. Pushes then rely on whatever
/// credentials git already has.
fn stored_token() -> Option<deploy::Token> {
    #[cfg(feature = "editor")]
    {
        profileit::github::load()
    }
    #[cfg(not(feature = "editor"))]
    {
        None
    }
}

fn run_init(path: &Path) -> ExitCode {
    let strings = cli_strings(path, None);
    match init::init(path) {
        Ok(()) => {
            println!(
                "{}",
                Message::new("cli.initOk")
                    .with("path", path.display())
                    .render(&strings)
            );
            ExitCode::SUCCESS
        }
        Err(err) => {
            say(Message::new("cli.initFailed").with("detail", err), &strings);
            ExitCode::FAILURE
        }
    }
}

#[cfg(feature = "editor")]
fn run_edit(path: &Path) -> ExitCode {
    // We don't block on validation failures here — a broken config is exactly
    // when the editor is needed, so errors are shown and fixed inside it instead.
    match profileit::serve::run(path) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            say(
                Message::new("cli.editFailed").with("detail", err),
                &cli_strings(path, None),
            );
            ExitCode::FAILURE
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Output
// ─────────────────────────────────────────────────────────────────────────────

fn report_load_error(path: &Path, err: LoadError) -> ExitCode {
    let strings = cli_strings(path, None);

    if let LoadError::Invalid(diagnostics) = &err {
        report(diagnostics, &strings);
        eprintln!();
    }
    eprintln!("{}", err.message().render(&strings));
    ExitCode::FAILURE
}

/// The language to use for terminal output.
///
/// Uses `PROFILEIT_LANG` if set, otherwise falls back to this card's default
/// language — better than showing English errors to someone working on a
/// Korean card.
fn cli_strings(path: &Path, config: Option<&profileit::config::Config>) -> Strings {
    let root = project_root(path);
    let chosen = std::env::var("PROFILEIT_LANG")
        .ok()
        .or_else(|| config.map(|c| c.default_language().to_string()))
        .unwrap_or_else(|| "ko".to_string());

    Strings::load(&chosen, root)
        .or_else(|_| Strings::load("ko", root))
        .unwrap_or_default()
}

fn report(diagnostics: &[Diagnostic], strings: &Strings) {
    for d in diagnostics {
        let tag = match d.severity {
            Severity::Error => strings.get("cli.error"),
            Severity::Warning => strings.get("cli.warning"),
        };
        eprintln!("[{tag}] {}: {}", d.path, d.message.render(strings));
    }
}
