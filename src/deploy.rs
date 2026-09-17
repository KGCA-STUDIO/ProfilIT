//! GitHub Pages 배포.
//!
//! `dist/` 를 리포지터리의 배포 브랜치(기본 `gh-pages`)로 올립니다.
//!
//! 인증은 두 길입니다. GitHub 을 연결해 두었으면 저장된 토큰을 `GIT_ASKPASS`
//! 로 넘기고, 아니면 이미 설정된 git 자격증명에 맡깁니다. 어느 쪽이든 토큰이
//! 명령줄이나 리모트 주소에 박히지 않습니다.
//!
//! 작업 트리는 건드리지 않습니다. 임시 인덱스에 `dist/` 를 담아 트리를 만들고
//! `commit-tree` 로 커밋을 빚어 푸시합니다 — 브랜치를 체크아웃하지 않으므로
//! 편집 중이던 파일이 사라지거나 섞이지 않습니다.

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::Config;
use crate::message::Message;

/// GitHub 토큰.
///
/// `Debug` 를 직접 구현해 값이 로그나 오류 메시지에 딸려 나가지 않게 합니다.
/// 파생 구현을 그대로 뒀다면 어딘가의 `{:?}` 한 번으로 새어 나갑니다.
#[derive(Clone)]
pub struct Token(String);

impl Token {
    pub fn new(value: impl Into<String>) -> Token {
        Token(value.into())
    }

    /// 값을 꺼냅니다. **git 에 넘기거나 GitHub 에 보낼 때만** 쓰세요.
    /// 로그나 오류 메시지에는 절대 넣지 않습니다.
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
    /// 올라간 파일 수.
    pub files: usize,
    /// 완성된 Pages 주소. 리모트가 GitHub 이 아니면 `None`.
    pub pages_url: Option<String>,
    /// Pages 를 아직 켜지 않았을 수 있으니 안내할 설정 화면 주소.
    pub settings_url: Option<String>,
    /// 올라가긴 했지만 알아둬야 할 것들.
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
    /// 화면에 내보낼 문구. 어느 언어로 만들지는 부르는 쪽이 정합니다.
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

/// git 단계 이름. 키로 두어 화면 언어에 맞춰 번역됩니다.
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
// 배포
// ─────────────────────────────────────────────────────────────────────────────

/// 저장소가 아니면 만듭니다. 명함 폴더는 보통 새 폴더라 흔한 경우입니다.
pub fn ensure_repository(root: &Path) -> Result<(), Error> {
    if git(root, &["rev-parse", "--git-dir"]).is_ok() {
        return Ok(());
    }
    git(root, &["init"]).map_err(|m| Error::Git { step: "init", message: m })?;
    Ok(())
}

/// 리모트를 가리키게 합니다. 있으면 주소만 바꿉니다.
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

/// `dist/` 를 배포 브랜치로 올립니다.
///
/// `token` 이 있으면 그것으로 인증합니다. 없으면 이미 설정된 git 자격증명에
/// 맡깁니다 — 터미널에서 쓰던 방식 그대로입니다.
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

    // 리모트 브랜치를 최신으로. 없는 브랜치면 실패하는데, 첫 배포라 정상입니다.
    let _ = git(root, &["fetch", &remote, &branch]);

    let index = git_dir.join("profileit-deploy-index");
    let _ = std::fs::remove_file(&index);

    // 작업 디렉터리를 dist 로 옮기므로 경로를 모두 절대 경로로 굳힙니다.
    let work_tree = dist.canonicalize()?;

    // work-tree 를 dist 로 두면 파일들이 저장소 루트에 놓입니다.
    // -f 는 루트 .gitignore 의 `dist/` 규칙을 넘기기 위한 것입니다.
    git_with_index(&git_dir, &index, &work_tree, &["add", "-A", "-f", "."])
        .map_err(|m| Error::Git { step: "stage", message: m })?;

    let tree = git_with_index(&git_dir, &index, &work_tree, &["write-tree"])
        .map_err(|m| Error::Git { step: "tree", message: m })?;
    let _ = std::fs::remove_file(&index);

    let message = format!("명함 갱신 — {}", timestamp());
    let mut commit_args = vec!["commit-tree", tree.as_str(), "-m", message.as_str()];

    // 이전 배포가 있으면 그 위에 쌓습니다. 히스토리가 이어져야 푸시가
    // fast-forward 로 통과합니다.
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

    // base_url 이 실제 주소와 다르면 og:url·canonical 이 엉뚱한 곳을 가리킵니다.
    // 화면상으로는 멀쩡해서 공유해보기 전까지 모르는 종류의 고장입니다.
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

/// 끝 슬래시와 대소문자만 다른 주소는 같은 것으로 봅니다.
fn same_url(a: &str, b: &str) -> bool {
    a.trim_end_matches('/').eq_ignore_ascii_case(b.trim_end_matches('/'))
}

/// GitHub Pages 가 요구하는 파일을 `dist/` 에 넣습니다.
fn prepare(config: &Config, dist: &Path) -> Result<(), Error> {
    // 이게 없으면 GitHub 이 Jekyll 을 돌려서 `_` 로 시작하는 파일·폴더를
    // 통째로 무시합니다. 지금은 해당 파일이 없지만 나중에 생기면 원인을
    // 찾기 어려운 종류의 고장이라 미리 막습니다.
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
    // 외부 크레이트 없이 찍습니다. 사람이 읽을 표식이면 충분합니다.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

// ─────────────────────────────────────────────────────────────────────────────
// git 부르기
// ─────────────────────────────────────────────────────────────────────────────

fn git(root: &Path, args: &[&str]) -> Result<String, String> {
    run(Command::new("git").current_dir(root).args(args))
}

/// 푸시.
///
/// 토큰이 있으면 `GIT_ASKPASS` 로 넘깁니다. 명령줄에 붙이면(`https://토큰@...`)
/// 프로세스 목록과 git 오류 메시지에 그대로 노출됩니다. 임시 스크립트는 값을
/// 담지 않고 환경 변수를 되읽기만 하므로 디스크에도 남지 않습니다.
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

    // 사용자 이름은 주소에 박아 둡니다. 비밀번호만 askpass 로 받으면 됩니다.
    let target = match parse_github(remote_url) {
        Some(repo) => format!("https://x-access-token@github.com/{}/{}.git", repo.owner, repo.name),
        None => remote_url.to_string(),
    };

    let helper = AskPass::create(git_dir)?;

    let result = run(Command::new("git")
        .current_dir(root)
        .env("GIT_ASKPASS", helper.path())
        .env("PROFILEIT_GIT_TOKEN", token.expose())
        // 자격증명을 못 얻었을 때 터미널 입력을 기다리며 멈추지 않게 합니다.
        .env("GIT_TERMINAL_PROMPT", "0")
        .args(["push", &target, refspec]));

    result
        .map(|_| ())
        .map_err(|m| Error::Git { step: "push", message: scrub(&m) })
}

/// 토큰이 오류 메시지에 섞여 나오지 않게 지웁니다.
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

/// git 이 비밀번호를 물어볼 때 실행되는 임시 스크립트.
///
/// **공용 임시 폴더에 두지 않습니다.** 유닉스의 `/tmp` 는 누구나 쓸 수 있어서,
/// 이름이 예측 가능하면 공격자가 미리 심볼릭 링크를 걸어 엉뚱한 파일을 덮어쓰게
/// 하거나, 우리가 쓴 뒤 git 이 실행하기 전에 내용을 바꿔치기해 사용자 권한으로
/// 임의 코드를 돌릴 수 있습니다.
///
/// 그래서 저장소의 `.git` 안에 만듭니다. 이미 임시 인덱스를 두는 곳이고,
/// 사용자 소유라 남이 끼어들 수 없습니다. 이름에는 프로세스 번호와 시각을
/// 붙이고 `create_new` 로 만들어, 같은 이름이 이미 있으면 따라가지 않고
/// 실패합니다.
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
            // 만드는 순간부터 주인만 읽고 실행할 수 있게 합니다. 만든 뒤에
            // 권한을 고치면 그 사이에 잠깐 열려 있습니다.
            options.mode(0o700);
        }

        // create_new 는 O_EXCL 이라 이미 있는 항목(심볼릭 링크 포함)을 따라가지
        // 않고 실패합니다.
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

/// 임시 인덱스와 다른 work-tree 로 git 을 부릅니다.
///
/// 작업 중인 인덱스를 건드리지 않는 것이 핵심입니다. 그냥 `git add` 를 쓰면
/// 편집 중이던 스테이징이 배포 때마다 뒤엎힙니다.
///
/// `git_dir` 과 `work_tree` 는 **반드시 절대 경로**여야 합니다. 작업 디렉터리를
/// dist 로 바꾸기 때문에, 상대 경로를 주면 git 이 `dist/.git` 을 찾습니다.
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
// GitHub 주소 알아내기
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, PartialEq)]
pub struct Repo {
    pub owner: String,
    pub name: String,
}

impl Repo {
    /// 이 리포지터리가 받게 될 Pages 주소.
    pub fn pages_url(&self) -> String {
        // `<사용자>.github.io` 리포지터리만 도메인 루트를 씁니다.
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

/// 리모트 주소에서 소유자와 이름을 뽑습니다. GitHub 이 아니면 `None`.
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

    /// `<사용자>.github.io` 는 도메인 루트에 놓입니다. 이걸 틀리면 안내하는
    /// 주소가 404 가 됩니다.
    #[test]
    fn user_site_repository_lives_at_the_domain_root() {
        let user_site = parse_github("https://github.com/Aiden/Aiden.github.io").unwrap();
        assert_eq!(user_site.pages_url(), "https://aiden.github.io/");

        let project = parse_github("https://github.com/Aiden/card").unwrap();
        assert_eq!(project.pages_url(), "https://aiden.github.io/card/");
    }

    /// askpass 스크립트는 공용 임시 폴더에 두지 않습니다. 유닉스의 /tmp 는
    /// 누구나 쓸 수 있어서, 이름이 예측 가능하면 심볼릭 링크를 걸어두거나
    /// 내용을 바꿔치기해 사용자 권한으로 코드를 돌릴 수 있습니다.
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

        // 이름이 고정되어 있으면 남이 미리 자리를 잡아둘 수 있습니다.
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        assert!(name.contains(&std::process::id().to_string()), "이름: {name}");

        // 같은 자리를 두 번 만들면 안 됩니다 — 하나가 다른 하나를 덮어씁니다.
        let second = AskPass::create(&dir).expect("두 번째");
        assert_ne!(second.path(), path);

        drop(helper);
        assert!(!path.exists(), "쓰고 나서 지워지지 않았습니다");
    }

    /// 이미 자리를 차지한 항목(심볼릭 링크 포함)을 따라가지 않고 실패해야 합니다.
    #[test]
    fn askpass_refuses_to_overwrite_an_existing_path() {
        let dir = std::env::temp_dir().join("profileit-askpass-clash");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let helper = AskPass::create(&dir).expect("askpass");
        let path = helper.path().to_path_buf();
        std::mem::forget(helper); // 파일을 남겨둔 채 같은 경로를 다시 노립니다

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

    /// Jekyll 을 끄지 않으면 `_` 로 시작하는 파일이 조용히 사라집니다.
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
