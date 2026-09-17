//! GitHub Pages deployment.
//!
//! Pushes `dist/` to the repository's deploy branch (`gh-pages` by default).
//!
//! There are two authentication paths. If GitHub is connected, the stored
//! token is passed through `GIT_ASKPASS`; otherwise we fall back to whatever
//! git credentials are already configured. Either way, the token never ends
//! up in the command line or the remote URL.
//!
//! The working tree is never touched. We build a tree from `dist/` in a
//! temporary index and forge a commit with `commit-tree`, then push that —
//! since we never check out the branch, files being edited can't disappear
//! or get mixed in.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::Config;
use crate::message::Message;

/// A GitHub token.
///
/// `Debug` is implemented by hand so the value never tags along in logs or
/// error messages. Leaving the derived impl in place would leak it the first
/// time something does a stray `{:?}`.
#[derive(Clone)]
pub struct Token(String);

impl Token {
    pub fn new(value: impl Into<String>) -> Token {
        Token(value.into())
    }

    /// Returns the raw value. Use this **only** to hand it to git or GitHub.
    /// Never put it in a log line or error message.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Token(***)")
    }
}

#[derive(Debug)]
pub struct Outcome {
    pub branch: String,
    pub commit: String,
    /// Number of files pushed.
    pub files: usize,
    /// The resulting Pages URL. `None` if the remote isn't GitHub.
    pub pages_url: Option<String>,
    /// Settings-page URL to point to, since Pages might not be enabled yet.
    pub settings_url: Option<String>,
    /// The push succeeded, but here's what's worth knowing about it.
    pub warnings: Vec<Message>,
}

#[derive(Debug)]
pub enum Error {
    GitMissing,
    NotARepository(PathBuf),
    NoRemote(String),
    NothingToDeploy(PathBuf),
    Git { step: &'static str, message: String },
    Io(std::io::Error),
}

impl Error {
    /// The message to show the user. The caller decides what language to render it in.
    pub fn message(&self) -> Message {
        match self {
            Error::GitMissing => Message::new("msg.deploy.gitMissing"),
            Error::NotARepository(path) => {
                Message::new("msg.deploy.notARepository").with("path", path.display())
            }
            Error::NoRemote(name) => Message::new("msg.deploy.noRemote").with("remote", name),
            Error::NothingToDeploy(path) => {
                Message::new("msg.deploy.nothingToDeploy").with("path", path.display())
            }
            Error::Git { step, message } => Message::new("msg.deploy.gitFailed")
                .with_key("step", step_key(step))
                .with("detail", message),
            Error::Io(err) => Message::new("msg.deploy.io").with("detail", err),
        }
    }
}

/// Name of a git step. Kept as a key so it gets translated to the UI language.
pub fn step_key(step: &str) -> &'static str {
    match step {
        "stage" => "msg.deploy.step.stage",
        "tree" => "msg.deploy.step.tree",
        "commit" => "msg.deploy.step.commit",
        "push" => "msg.deploy.step.push",
        "init" => "msg.deploy.step.init",
        "remote" => "msg.deploy.step.remote",
        _ => "msg.deploy.step.unknown",
    }
}

impl From<std::io::Error> for Error {
    fn from(err: std::io::Error) -> Self {
        Error::Io(err)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Deploy
// ─────────────────────────────────────────────────────────────────────────────

/// Creates a repository if there isn't one already. Card folders are usually
/// fresh folders, so this is the common case.
pub fn ensure_repository(root: &Path) -> Result<(), Error> {
    if git(root, &["rev-parse", "--git-dir"]).is_ok() {
        return Ok(());
    }
    git(root, &["init"]).map_err(|m| Error::Git { step: "init", message: m })?;
    Ok(())
}

/// Points the remote at the given URL. If it already exists, just updates the URL.
pub fn set_remote(root: &Path, name: &str, url: &str) -> Result<(), Error> {
    ensure_repository(root)?;

    let result = if git(root, &["remote", "get-url", name]).is_ok() {
        git(root, &["remote", "set-url", name, url])
    } else {
        git(root, &["remote", "add", name, url])
    };

    result
        .map(|_| ())
        .map_err(|m| Error::Git { step: "remote", message: m })
}

/// Pushes `dist/` to the deploy branch.
///
/// If `token` is given, it's used to authenticate. Otherwise we fall back to
/// whatever git credentials are already configured — the same as using the
/// terminal directly.
pub fn publish(
    config: &Config,
    root: &Path,
    dist: &Path,
    token: Option<&Token>,
) -> Result<Outcome, Error> {
    let settings = &config.deploy;
    let branch = settings.branch.clone();
    let remote = settings.remote.clone();

    if !dist.exists() || std::fs::read_dir(dist)?.next().is_none() {
        return Err(Error::NothingToDeploy(dist.to_path_buf()));
    }

    let git_dir = git_dir(root)?;
    let remote_url = remote_url(root, &remote)?;

    prepare(config, dist)?;

    // Bring the remote branch up to date. This fails if the branch doesn't
    // exist yet, which is expected on the first deploy.
    let _ = git(root, &["fetch", &remote, &branch]);

    let index = git_dir.join("profileit-deploy-index");
    let _ = std::fs::remove_file(&index);

    // We're switching the working directory to dist, so pin every path to
    // an absolute one.
    let work_tree = dist.canonicalize()?;

    // Setting the work-tree to dist puts these files at the repo root.
    // -f overrides the root .gitignore's `dist/` rule.
    git_with_index(&git_dir, &index, &work_tree, &["add", "-A", "-f", "."])
        .map_err(|m| Error::Git { step: "stage", message: m })?;

    let tree = git_with_index(&git_dir, &index, &work_tree, &["write-tree"])
        .map_err(|m| Error::Git { step: "tree", message: m })?;
    let _ = std::fs::remove_file(&index);

    let message = format!("명함 갱신 — {}", timestamp());
    let mut commit_args = vec!["commit-tree", tree.as_str(), "-m", message.as_str()];

    // If there was a previous deploy, build on top of it. History needs to
    // stay connected for the push to go through as a fast-forward.
    let parent = git(root, &["rev-parse", "--verify", &format!("{remote}/{branch}")]).ok();
    if let Some(parent) = &parent {
        commit_args.push("-p");
        commit_args.push(parent);
    }

    let commit = git(root, &commit_args).map_err(|m| Error::Git {
        step: "commit",
        message: m,
    })?;

    let refspec = format!("{commit}:refs/heads/{branch}");
    push(root, &git_dir, &remote, &remote_url, &refspec, token)?;

    let (pages_url, settings_url) = match parse_github(&remote_url) {
        Some(repo) => (
            Some(settings.cname.clone().map_or_else(
                || repo.pages_url(),
                |domain| format!("https://{domain}/"),
            )),
            Some(repo.settings_url()),
        ),
        None => (None, None),
    };

    let mut warnings = Vec::new();

    // If base_url doesn't match the actual URL, og:url/canonical point
    // somewhere wrong. The page looks fine either way, so this kind of
    // breakage stays invisible until someone actually shares the link.
    if let (Some(actual), Some(configured)) = (&pages_url, &config.site.base_url) {
        if !same_url(actual, configured) {
            warnings.push(
                Message::new("msg.deploy.baseUrlMismatch")
                    .with("configured", configured)
                    .with("actual", actual),
            );
        }
    }

    Ok(Outcome {
        branch,
        commit,
        files: count_files(dist),
        pages_url,
        settings_url,
        warnings,
    })
}

/// URLs that only differ by a trailing slash or case are treated as the same.
fn same_url(a: &str, b: &str) -> bool {
    a.trim_end_matches('/').eq_ignore_ascii_case(b.trim_end_matches('/'))
}

/// Writes the files GitHub Pages expects into `dist/`.
fn prepare(config: &Config, dist: &Path) -> Result<(), Error> {
    // Without this, GitHub runs Jekyll, which silently ignores every file and
    // folder starting with `_`. We don't have any such files right now, but
    // this is the kind of failure that's hard to diagnose once we do, so we
    // head it off preemptively.
    std::fs::write(dist.join(".nojekyll"), b"")?;

    if let Some(domain) = &config.deploy.cname {
        std::fs::write(dist.join("CNAME"), domain.as_bytes())?;
    }
    Ok(())
}

fn count_files(dir: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => count_files(&entry.path()),
            _ => 1,
        })
        .sum()
}

fn timestamp() -> String {
    // Stamped without pulling in an external crate. A human-readable marker is all we need.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

// ─────────────────────────────────────────────────────────────────────────────
// Calling git
// ─────────────────────────────────────────────────────────────────────────────

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    run(Command::new("git").current_dir(root).args(args))
}

/// Push.
///
/// If a token is given, it's passed via `GIT_ASKPASS`. Embedding it in the
/// URL (`https://token@...`) would expose it in the process list and in git's
/// own error messages. The temporary script doesn't hold the value itself —
/// it only reads it back from an environment variable — so it never touches disk either.
fn push(
    root: &Path,
    git_dir: &Path,
    remote: &str,
    remote_url: &str,
    refspec: &str,
    token: Option<&Token>,
) -> Result<(), Error> {
    let Some(token) = token else {
        return git(root, &["push", remote, refspec])
            .map(|_| ())
            .map_err(|m| Error::Git { step: "push", message: m });
    };

    // The username is baked into the URL; only the password needs to come from askpass.
    let target = match parse_github(remote_url) {
        Some(repo) => format!("https://x-access-token@github.com/{}/{}.git", repo.owner, repo.name),
        None => remote_url.to_string(),
    };

    let helper = AskPass::create(git_dir)?;

    let result = run(Command::new("git")
        .current_dir(root)
        .env("GIT_ASKPASS", helper.path())
        .env("PROFILEIT_GIT_TOKEN", token.expose())
        // Stops git from hanging on a terminal prompt if it can't get credentials.
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(["push", &target, refspec]));

    result
        .map(|_| ())
        .map_err(|m| Error::Git { step: "push", message: scrub(&m) })
}

/// Strips the token out so it can't show up mixed into an error message.
fn scrub(message: &str) -> String {
    let mut cleaned = String::with_capacity(message.len());
    for part in message.split_whitespace() {
        if part.starts_with("ghp_") || part.starts_with("github_pat_") {
            cleaned.push_str("***");
        } else {
            cleaned.push_str(part);
        }
        cleaned.push(' ');
    }
    cleaned.trim_end().to_string()
}

/// The temporary script git runs when it asks for a password.
///
/// **This is never placed in a shared temp folder.** Unix's `/tmp` is
/// writable by anyone, so a predictable filename lets an attacker pre-plant a
/// symlink there to make us overwrite an arbitrary file, or swap the content
/// out after we write it but before git executes it, running arbitrary code
/// under the user's privileges.
///
/// So it's created inside the repository's `.git` instead — a place that
/// already holds the temporary index and is owned by the user, so nobody
/// else can interfere. The filename includes the process ID and a timestamp,
/// and it's created with `create_new`, so if a file with that name already
/// exists, creation fails instead of following it.
struct AskPass(PathBuf);

impl AskPass {
    fn create(git_dir: &Path) -> Result<AskPass, Error> {
        #[cfg(windows)]
        let (extension, body) = ("bat", "@echo off\r\necho %PROFILEIT_GIT_TOKEN%\r\n");
        #[cfg(not(windows))]
        let (extension, body) = ("sh", "#!/bin/sh\nprintf '%s' \"$PROFILEIT_GIT_TOKEN\"\n");

        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = git_dir.join(format!(
            "profileit-askpass-{}-{unique}.{extension}",
            std::process::id()
        ));

        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);

        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            // Make it readable and executable by the owner only, from the moment
            // it's created. Fixing permissions after the fact would leave it
            // briefly world-accessible in between.
            options.mode(0o700);
        }

        // create_new maps to O_EXCL, so an existing entry (including a symlink)
        // is never followed — creation just fails instead.
        let mut file = options.open(&path)?;
        std::io::Write::write_all(&mut file, body.as_bytes())?;

        Ok(AskPass(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for AskPass {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Runs git against a temporary index and a different work-tree.
///
/// The point is to leave the real index untouched. A plain `git add` would
/// stomp on whatever was staged for editing every time we deploy.
///
/// `git_dir` and `work_tree` **must be absolute paths**. Since we change the
/// working directory to dist, a relative path would make git look for `dist/.git`.
fn git_with_index(
    git_dir: &Path,
    index: &Path,
    work_tree: &Path,
    args: &[&str],
) -> Result<String, String> {
    run(Command::new("git")
        .current_dir(work_tree)
        .env("GIT_INDEX_FILE", index)
        .env("GIT_DIR", git_dir)
        .env("GIT_WORK_TREE", work_tree)
        .args(args))
}

fn run(command: &mut Command) -> Result<String, String> {
    let output = command.output().map_err(|err| err.to_string())?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        return Err(if stderr.is_empty() { stdout } else { stderr });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn git_dir(root: &Path) -> Result<PathBuf, Error> {
    if Command::new("git").arg("--version").output().is_err() {
        return Err(Error::GitMissing);
    }
    match git(root, &["rev-parse", "--absolute-git-dir"]) {
        Ok(dir) => Ok(PathBuf::from(dir)),
        Err(_) => Err(Error::NotARepository(root.to_path_buf())),
    }
}

fn remote_url(root: &Path, remote: &str) -> Result<String, Error> {
    git(root, &["remote", "get-url", remote]).map_err(|_| Error::NoRemote(remote.to_string()))
}

// ─────────────────────────────────────────────────────────────────────────────
// Figuring out the GitHub URL
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq)]
pub struct Repo {
    pub owner: String,
    pub name: String,
}

impl Repo {
    /// The Pages URL this repository will get.
    pub fn pages_url(&self) -> String {
        // Only a `<user>.github.io` repository lives at the domain root.
        if self.name.eq_ignore_ascii_case(&format!("{}.github.io", self.owner)) {
            format!("https://{}.github.io/", self.owner.to_lowercase())
        } else {
            format!("https://{}.github.io/{}/", self.owner.to_lowercase(), self.name)
        }
    }

    pub fn settings_url(&self) -> String {
        format!("https://github.com/{}/{}/settings/pages", self.owner, self.name)
    }
}

/// Extracts the owner and name from a remote URL. `None` if it isn't GitHub.
pub fn parse_github(url: &str) -> Option<Repo> {
    let url = url.trim().trim_end_matches('/');
    let rest = if let Some(rest) = url.strip_prefix("git@github.com:") {
        rest
    } else if let Some(rest) = url.strip_prefix("https://github.com/") {
        rest
    } else if let Some(rest) = url.strip_prefix("ssh://git@github.com/") {
        rest
    } else {
        return None;
    };

    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let (owner, name) = rest.split_once('/')?;

    if owner.is_empty() || name.is_empty() {
        return None;
    }
    Some(Repo {
        owner: owner.to_string(),
        name: name.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_remote_shape() {
        let expected = Repo {
            owner: "aiden".into(),
            name: "card".into(),
        };
        for url in [
            "https://github.com/aiden/card.git",
            "https://github.com/aiden/card",
            "git@github.com:aiden/card.git",
            "ssh://git@github.com/aiden/card.git",
            "https://github.com/aiden/card/",
        ] {
            assert_eq!(parse_github(url).as_ref(), Some(&expected), "{url}");
        }
    }

    #[test]
    fn ignores_non_github_remotes() {
        assert!(parse_github("https://gitlab.com/aiden/card.git").is_none());
        assert!(parse_github("/srv/git/card.git").is_none());
        assert!(parse_github("https://github.com/aiden").is_none());
    }

    /// `<user>.github.io` lives at the domain root. Getting this wrong means
    /// the URL we point the user to comes back 404.
    #[test]
    fn user_site_repository_lives_at_the_domain_root() {
        let user_site = parse_github("https://github.com/Aiden/Aiden.github.io").unwrap();
        assert_eq!(user_site.pages_url(), "https://aiden.github.io/");

        let project = parse_github("https://github.com/Aiden/card").unwrap();
        assert_eq!(project.pages_url(), "https://aiden.github.io/card/");
    }

    /// The askpass script is never placed in a shared temp folder. Unix's /tmp
    /// is writable by anyone, so a predictable filename would let someone plant
    /// a symlink there or swap the content to run code under the user's privileges.
    #[test]
    fn askpass_lives_in_the_repository_not_shared_temp() {
        let dir = std::env::temp_dir().join("profileit-askpass-home");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let helper = AskPass::create(&dir).expect("askpass");
        let path = helper.path().to_path_buf();

        assert!(path.starts_with(&dir), "저장소 밖에 만들어졌습니다: {path:?}");
        assert_ne!(
            path.parent(),
            Some(std::env::temp_dir().as_path()),
            "공용 임시 폴더에 그대로 놓였습니다"
        );

        // If the name were fixed, someone else could stake out that spot ahead of time.
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        assert!(name.contains(&std::process::id().to_string()), "이름: {name}");

        // Creating the same spot twice must not happen — one would overwrite the other.
        let second = AskPass::create(&dir).expect("두 번째");
        assert_ne!(second.path(), path);

        drop(helper);
        assert!(!path.exists(), "쓰고 나서 지워지지 않았습니다");
    }

    /// Must fail rather than follow an entry that's already there (including a symlink).
    #[test]
    fn askpass_refuses_to_overwrite_an_existing_path() {
        let dir = std::env::temp_dir().join("profileit-askpass-clash");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let helper = AskPass::create(&dir).expect("askpass");
        let path = helper.path().to_path_buf();
        std::mem::forget(helper); // Leave the file behind and target the same path again

        let mut options = std::fs::OpenOptions::new();
        let clash = options.write(true).create_new(true).open(&path);
        assert!(clash.is_err(), "이미 있는 경로를 덮어썼습니다");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn trailing_slash_does_not_count_as_a_mismatch() {
        assert!(same_url("https://a.github.io/card/", "https://a.github.io/card"));
        assert!(same_url("https://A.github.io/card", "https://a.github.io/card/"));
        assert!(!same_url("https://a.github.io/card/", "https://a.github.io/other/"));
    }

    #[test]
    fn counts_files_recursively() {
        let dir = std::env::temp_dir().join("profileit-deploy-count");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("en")).unwrap();
        std::fs::write(dir.join("index.html"), "a").unwrap();
        std::fs::write(dir.join("styles.css"), "b").unwrap();
        std::fs::write(dir.join("en/index.html"), "c").unwrap();

        assert_eq!(count_files(&dir), 3);
    }

    /// Without disabling Jekyll, files starting with `_` silently disappear.
    #[test]
    fn prepare_writes_nojekyll_and_cname() {
        let dir = std::env::temp_dir().join("profileit-deploy-prepare");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let mut config: Config =
            toml::from_str(include_str!("../profile.toml")).expect("예시 설정");
        config.deploy.cname = Some("card.example.com".into());

        prepare(&config, &dir).unwrap();

        assert!(dir.join(".nojekyll").exists());
        assert_eq!(
            std::fs::read_to_string(dir.join("CNAME")).unwrap(),
            "card.example.com"
        );
    }
}
