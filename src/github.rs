//! GitHub integration.
//!
//! Tokens live in the **OS credential store** (Windows Credential Manager,
//! macOS Keychain, Linux Secret Service). They're never written in plaintext
//! to a config file or the app folder, so sharing the whole project folder —
//! or accidentally committing it — doesn't leak the token along with it.
//!
//! A real OAuth login (an "Authorize" button) would need a registered OAuth
//! App. Once we have a registered `client_id`, the device-authorization flow
//! can be added here, and everything else (listing/creating repos, enabling
//! Pages) works as-is.

use serde::{Deserialize, Serialize};

use crate::deploy::Token;
use crate::message::Message;

const SERVICE: &str = "ProfileIT";
const ACCOUNT: &str = "github-token";
const API: &str = "https://api.github.com";
const USER_AGENT: &str = "ProfileIT";

/// Permissions the token needs. Appended to the link to pre-fill GitHub's token page.
pub const SCOPES: &str = "repo";

/// Token-creation page, with scope and name already filled in.
pub fn token_page_url() -> String {
    format!("https://github.com/settings/tokens/new?scopes={SCOPES}&description=ProfileIT")
}

// ─────────────────────────────────────────────────────────────────────────────
// Token
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
        // If there was nothing to begin with, that's the same outcome as deleting it.
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
    /// The https URL git will use.
    pub clone_url: String,
}

#[derive(Debug, Serialize)]
pub struct PagesInfo {
    pub url: Option<String>,
    pub branch: Option<String>,
}

/// Checks that the token is valid and reports which account it belongs to.
pub fn whoami(token: &Token) -> Result<Account, Error> {
    let value: serde_json::Value = get(token, "/user")?;
    Ok(Account {
        login: string(&value, "login").unwrap_or_default(),
        name: string(&value, "name"),
        avatar_url: string(&value, "avatar_url"),
    })
}

/// Repositories the token can push to, most recently touched first.
pub fn list_repos(token: &Token) -> Result<Vec<RepoSummary>, Error> {
    let mut repos = Vec::new();

    // 100 per page, up to 3 pages. Beyond that, typing the name directly
    // is faster than picking it out of a list anyway.
    for page in 1..=3 {
        let path =
            format!("/user/repos?per_page=100&page={page}&sort=updated&affiliation=owner,collaborator");
        let value: serde_json::Value = get(token, &path)?;
        let Some(items) = value.as_array() else { break };
        if items.is_empty() {
            break;
        }

        for item in items {
            // Listing a repo the user can't push to would only fail after they pick it.
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

/// Creates a new repository.
pub fn create_repo(token: &Token, name: &str, private: bool) -> Result<RepoSummary, Error> {
    let body = serde_json::json!({
        "name": name,
        "private": private,
        // The repo description stays on GitHub for anyone to see. Following the
        // app's UI language would make the same repo read differently depending
        // on who's looking, and the reader isn't necessarily an app user anyway,
        // so we hardcode English.
        "description": "An online business card made with ProfileIT",
        // An empty repo needs at least one commit before Pages can be enabled.
        "auto_init": true,
    });

    let value = post(token, "/user/repos", body)?;
    to_repo(&value).ok_or_else(|| Error::Api {
        status: 0,
        message: Message::new("msg.gh.badRepo"),
    })
}

/// Enables Pages. If it's already enabled, just switches the branch.
pub fn enable_pages(
    token: &Token,
    owner: &str,
    repo: &str,
    branch: &str,
) -> Result<PagesInfo, Error> {
    let source = serde_json::json!({ "branch": branch, "path": "/" });

    // POST creates it the first time; if it already exists we get a 409, so we PUT instead.
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
// Requests
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
            // GitHub tells us why in JSON. Showing that as-is is far more useful
            // than a generic "request failed".
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
                    // Use GitHub's own description verbatim. We can't translate it,
                    // but it's still far more useful than a generic "request failed".
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
// Errors
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum Error {
    Keyring(String),
    Network(String),
    Api { status: u16, message: Message },
}

impl Error {
    /// The message to show the user. The caller decides what language to render it in.
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

    /// The token must never end up tagging along in logs or error messages.
    #[test]
    fn token_never_prints_itself() {
        let token = Token::new("ghp_superSecretValue123");

        assert_eq!(format!("{token:?}"), "Token(***)");
        assert!(!format!("{token:#?}").contains("superSecret"));

        // The same must hold even when it's wrapped inside a struct.
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

    /// A response missing required fields must not produce a bogus repository.
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
