//! GitHub 연동.
//!
//! 토큰은 **OS 자격증명 저장소**에 둡니다 (Windows 자격 증명 관리자, macOS
//! 키체인, Linux Secret Service). 설정 파일이나 앱 폴더에 평문으로 남기지
//! 않으므로, 프로젝트 폴더를 통째로 공유하거나 실수로 커밋해도 토큰은
//! 따라가지 않습니다.
//!
//! 진짜 OAuth 로그인(Authorize 버튼)은 OAuth App 등록이 필요합니다. 등록된
//! `client_id` 가 생기면 기기 인증 흐름을 여기에 더하면 되고, 나머지(저장소
//! 목록·생성·Pages 활성화)는 그대로 씁니다.

use serde::{Deserialize, Serialize};

use crate::deploy::Token;
use crate::message::Message;

const SERVICE: &str = "ProfileIT";
const ACCOUNT: &str = "github-token";
const API: &str = "https://api.github.com";
const USER_AGENT: &str = "ProfileIT";

/// 토큰을 만들 때 필요한 권한. 링크에 붙여 GitHub 발급 화면을 미리 채웁니다.
pub const SCOPES: &str = "repo";

/// 토큰 발급 화면. 권한과 이름이 미리 채워집니다.
pub fn token_page_url() -> String {
    format!("https://github.com/settings/tokens/new?scopes={SCOPES}&description=ProfileIT")
}

// ─────────────────────────────────────────────────────────────────────────────
// 토큰
// ─────────────────────────────────────────────────────────────────────────────

pub fn store(token: &Token) -> Result<(), Error> {
    entry()?
        .set_password(token.expose())
        .map_err(|e| Error::Keyring(e.to_string()))
}

pub fn load() -> Option<Token> {
    entry().ok()?.get_password().ok().map(Token::new)
}

pub fn forget() -> Result<(), Error> {
    match entry()?.delete_credential() {
        Ok(()) => Ok(()),
        // 원래 없었으면 지운 것과 결과가 같습니다.
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(err) => Err(Error::Keyring(err.to_string())),
    }
}

fn entry() -> Result<keyring::Entry, Error> {
    keyring::Entry::new(SERVICE, ACCOUNT).map_err(|e| Error::Keyring(e.to_string()))
}

// ─────────────────────────────────────────────────────────────────────────────
// API
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct Account {
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoSummary {
    pub full_name: String,
    pub name: String,
    pub owner: String,
    pub private: bool,
    pub default_branch: String,
    /// git 이 쓸 https 주소.
    pub clone_url: String,
}

#[derive(Debug, Serialize)]
pub struct PagesInfo {
    pub url: Option<String>,
    pub branch: Option<String>,
}

/// 토큰이 유효한지 보고 계정을 알려줍니다.
pub fn whoami(token: &Token) -> Result<Account, Error> {
    let value: serde_json::Value = get(token, "/user")?;
    Ok(Account {
        login: string(&value, "login").unwrap_or_default(),
        name: string(&value, "name"),
        avatar_url: string(&value, "avatar_url"),
    })
}

/// 쓰기 권한이 있는 저장소 목록. 최근에 손댄 것부터 옵니다.
pub fn list_repos(token: &Token) -> Result<Vec<RepoSummary>, Error> {
    let mut repos = Vec::new();

    // 한 번에 100개씩, 최대 3쪽. 그보다 많으면 목록에서 고르는 것보다
    // 이름을 직접 적는 편이 빠릅니다.
    for page in 1..=3 {
        let path =
            format!("/user/repos?per_page=100&page={page}&sort=updated&affiliation=owner,collaborator");
        let value: serde_json::Value = get(token, &path)?;
        let Some(items) = value.as_array() else { break };
        if items.is_empty() {
            break;
        }

        for item in items {
            // 쓸 수 없는 저장소를 목록에 올리면 고른 뒤에야 실패합니다.
            let can_push = item
                .get("permissions")
                .and_then(|p| p.get("push"))
                .and_then(serde_json::Value::as_bool)
                .unwrap_or(false);
            if !can_push {
                continue;
            }
            if let Some(repo) = to_repo(item) {
                repos.push(repo);
            }
        }

        if items.len() < 100 {
            break;
        }
    }

    Ok(repos)
}

/// 새 저장소를 만듭니다.
pub fn create_repo(token: &Token, name: &str, private: bool) -> Result<RepoSummary, Error> {
    let body = serde_json::json!({
        "name": name,
        "private": private,
        // 저장소 설명은 GitHub 에 그대로 남아 누구에게나 보입니다. 앱 화면 언어를
        // 따라가면 같은 저장소가 볼 때마다 달라질 수도 없고, 읽는 사람이 앱 사용자도
        // 아니므로 영어로 고정합니다.
        "description": "An online business card made with ProfileIT",
        // 빈 저장소면 Pages 를 켜기 전에 커밋이 하나는 있어야 합니다.
        "auto_init": true,
    });

    let value = post(token, "/user/repos", body)?;
    to_repo(&value).ok_or_else(|| Error::Api {
        status: 0,
        message: Message::new("msg.gh.badRepo"),
    })
}

/// Pages 를 켭니다. 이미 켜져 있으면 브랜치만 바꿉니다.
pub fn enable_pages(
    token: &Token,
    owner: &str,
    repo: &str,
    branch: &str,
) -> Result<PagesInfo, Error> {
    let source = serde_json::json!({ "branch": branch, "path": "/" });

    // 처음이면 POST 로 만들고, 이미 있으면 409 가 오므로 PUT 으로 고칩니다.
    let created = post(
        token,
        &format!("/repos/{owner}/{repo}/pages"),
        serde_json::json!({ "source": source }),
    );

    match created {
        Ok(value) => Ok(to_pages(&value)),
        Err(Error::Api { status: 409, .. }) => {
            put(
                token,
                &format!("/repos/{owner}/{repo}/pages"),
                serde_json::json!({ "source": source }),
            )?;
            pages_status(token, owner, repo)
        }
        Err(err) => Err(err),
    }
}

pub fn pages_status(token: &Token, owner: &str, repo: &str) -> Result<PagesInfo, Error> {
    let value: serde_json::Value = get(token, &format!("/repos/{owner}/{repo}/pages"))?;
    Ok(to_pages(&value))
}

// ─────────────────────────────────────────────────────────────────────────────
// 요청
// ─────────────────────────────────────────────────────────────────────────────

fn get(token: &Token, path: &str) -> Result<serde_json::Value, Error> {
    send(ureq::get(&format!("{API}{path}")), token, None)
}

fn post(token: &Token, path: &str, body: serde_json::Value) -> Result<serde_json::Value, Error> {
    send(ureq::post(&format!("{API}{path}")), token, Some(body))
}

fn put(token: &Token, path: &str, body: serde_json::Value) -> Result<serde_json::Value, Error> {
    send(ureq::put(&format!("{API}{path}")), token, Some(body))
}

fn send(
    request: ureq::Request,
    token: &Token,
    body: Option<serde_json::Value>,
) -> Result<serde_json::Value, Error> {
    let request = request
        .set("Authorization", &format!("Bearer {}", token.expose()))
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .set("User-Agent", USER_AGENT);

    let response = match body {
        Some(body) => request.send_json(body),
        None => request.call(),
    };

    match response {
        Ok(response) => response.into_json().map_err(|e| Error::Api {
            status: 0,
            message: Message::new("msg.gh.badResponse").with("detail", e),
        }),

        Err(ureq::Error::Status(status, response)) => {
            // GitHub 이 이유를 JSON 으로 알려줍니다. 그대로 보여주는 편이
            // "요청 실패" 보다 훨씬 쓸모 있습니다.
            let detail = response
                .into_json::<serde_json::Value>()
                .ok()
                .and_then(|v| string(&v, "message"))
                .unwrap_or_default();

            Err(Error::Api {
                status,
                message: match status {
                    401 => Message::new("msg.gh.badToken"),
                    403 if detail.contains("rate limit") => Message::new("msg.gh.rateLimit"),
                    403 => Message::new("msg.gh.forbidden").with("scopes", SCOPES),
                    404 => Message::new("msg.gh.notFound"),
                    // GitHub 이 준 설명을 그대로 씁니다. 번역할 수 없지만
                    // "요청 실패" 보다 훨씬 쓸모 있습니다.
                    _ if !detail.is_empty() => Message::new("msg.gh.detail").with("detail", detail),
                    _ => Message::new("msg.gh.status").with("status", status),
                },
            })
        }

        Err(err) => Err(Error::Network(err.to_string())),
    }
}

fn to_repo(value: &serde_json::Value) -> Option<RepoSummary> {
    Some(RepoSummary {
        full_name: string(value, "full_name")?,
        name: string(value, "name")?,
        owner: value.get("owner").and_then(|o| string(o, "login"))?,
        private: value
            .get("private")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        default_branch: string(value, "default_branch").unwrap_or_else(|| "main".into()),
        clone_url: string(value, "clone_url")?,
    })
}

fn to_pages(value: &serde_json::Value) -> PagesInfo {
    PagesInfo {
        url: string(value, "html_url"),
        branch: value
            .get("source")
            .and_then(|s| string(s, "branch"))
            .or_else(|| string(value, "branch")),
    }
}

fn string(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

// ─────────────────────────────────────────────────────────────────────────────
// 오류
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum Error {
    Keyring(String),
    Network(String),
    Api { status: u16, message: Message },
}

impl Error {
    /// 화면에 내보낼 문구. 어느 언어로 만들지는 부르는 쪽이 정합니다.
    pub fn message(&self) -> Message {
        match self {
            Error::Keyring(detail) => Message::new("msg.gh.keyring").with("detail", detail),
            Error::Network(detail) => Message::new("msg.gh.network").with("detail", detail),
            Error::Api { message, .. } => message.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 토큰이 로그나 오류 메시지에 딸려 나가면 안 됩니다.
    #[test]
    fn token_never_prints_itself() {
        let token = Token::new("ghp_superSecretValue123");

        assert_eq!(format!("{token:?}"), "Token(***)");
        assert!(!format!("{token:#?}").contains("superSecret"));

        // 구조체 안에 담겨도 마찬가지여야 합니다.
        #[derive(Debug)]
        struct Holder {
            #[allow(dead_code)]
            token: Token,
        }
        let held = Holder { token };
        assert!(!format!("{held:?}").contains("superSecret"));
    }

    #[test]
    fn token_page_url_asks_for_the_right_scope() {
        let url = token_page_url();
        assert!(url.starts_with("https://github.com/settings/tokens/new"));
        assert!(url.contains("scopes=repo"));
    }

    #[test]
    fn parses_a_repository_payload() {
        let value = serde_json::json!({
            "full_name": "aiden/card",
            "name": "card",
            "owner": { "login": "aiden" },
            "private": false,
            "default_branch": "main",
            "clone_url": "https://github.com/aiden/card.git",
        });

        let repo = to_repo(&value).expect("저장소");
        assert_eq!(repo.owner, "aiden");
        assert_eq!(repo.name, "card");
        assert_eq!(repo.clone_url, "https://github.com/aiden/card.git");
    }

    /// 필수 항목이 빠진 응답으로 엉뚱한 저장소를 만들면 안 됩니다.
    #[test]
    fn incomplete_repository_payload_is_rejected() {
        let value = serde_json::json!({ "name": "card" });
        assert!(to_repo(&value).is_none());
    }

    #[test]
    fn reads_pages_source_branch() {
        let value = serde_json::json!({
            "html_url": "https://aiden.github.io/card/",
            "source": { "branch": "gh-pages", "path": "/" },
        });

        let pages = to_pages(&value);
        assert_eq!(pages.url.as_deref(), Some("https://aiden.github.io/card/"));
        assert_eq!(pages.branch.as_deref(), Some("gh-pages"));
    }
}
