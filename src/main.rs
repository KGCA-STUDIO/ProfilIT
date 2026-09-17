//! profileit CLI.
//!
//! 실제 일은 전부 라이브러리(`src/lib.rs`)에 있습니다. 여기서는 명령을 고르고
//! 결과를 사람이 읽을 수 있게 출력하는 것만 합니다.

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

/// 한 줄을 화면 언어로 조립해 내보냅니다.
fn say(message: Message, strings: &Strings) {
    eprintln!("{}", message.render(strings));
}

// ─────────────────────────────────────────────────────────────────────────────
// 명령
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

/// 배포. **먼저 빌드합니다** — 고친 내용이 반영되지 않은 dist 를 올리는 것이
/// 가장 흔한 실수라서, 올릴 것을 그 자리에서 다시 만듭니다.
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

    // 연결해 둔 토큰이 있으면 씁니다. 없으면 git 자격증명에 맡깁니다.
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

/// 저장된 GitHub 토큰.
///
/// 편집기 기능을 끄고 빌드하면 자격증명 저장소를 읽을 수단이 없으므로 항상
/// `None` 입니다. 그때는 git 이 이미 갖고 있는 자격증명으로 푸시합니다.
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
    // 여기서는 검증 실패로 막지 않습니다 — 설정이 깨졌을 때야말로 편집기가
    // 필요하고, 편집기 안에서 오류를 보여주며 고칠 수 있습니다.
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
// 출력
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

/// 터미널에 쓸 언어.
///
/// `PROFILEIT_LANG` 이 있으면 그것을, 없으면 이 명함의 기본 언어를 씁니다 —
/// 한국어 명함을 쓰는 사람에게 영어 오류를 내미는 것보다 낫습니다.
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
