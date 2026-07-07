//! `mari auth <provider>` and `mari scope` per SPEC §5.1/§3.2.
//! Credentials are validated against the service, then saved to the source's
//! scope location with mode 0600 (SPEC §1.1). No env vars are read.

use crate::{config, workspace};
use anyhow::{anyhow, Result};
use serde_json::{json, Value};

pub struct AuthFlags {
    pub token: Option<String>,
    pub url: Option<String>,
    pub email: Option<String>,
    pub subdomain: Option<String>,
    pub key: Option<String>,
    pub secret: Option<String>,
    pub method: Option<String>,
}

pub const PROVIDERS: &[&str] = &[
    "confluence",
    "discord",
    "github",
    "google",
    "hubspot",
    "jira",
    "linear",
    "microsoft",
    "salesforce",
    "slack",
    "zendesk",
];

/// Auth provider `google` maps to source key `gdocs` (SPEC §5.1).
pub fn source_key(provider: &str) -> &str {
    if provider == "google" {
        "gdocs"
    } else {
        provider
    }
}

pub fn credential_path(provider: &str) -> std::path::PathBuf {
    let source = source_key(provider);
    let root = workspace::work_root();
    let global = workspace::source_scope(source) == "global";
    workspace::credentials_dir(global, &root).join(format!("{provider}.json"))
}

pub fn credential(provider: &str) -> Option<Value> {
    let p = credential_path(provider);
    let alt = config::mari_home()
        .join("credentials")
        .join(format!("{provider}.json"));
    for path in [p, alt] {
        if let Ok(s) = std::fs::read_to_string(&path) {
            if let Ok(v) = serde_json::from_str(&s) {
                return Some(v);
            }
        }
    }
    None
}

pub fn run(provider: &str, f: AuthFlags) -> Result<i32> {
    if !PROVIDERS.contains(&provider) {
        eprintln!(
            "unknown provider: {provider}\nproviders: {}",
            PROVIDERS.join(" ")
        );
        return Ok(2);
    }
    let _ = (&f.key, &f.secret, &f.method); // accepted per SPEC; unused by current providers
    let cred = match provider {
        "slack" => {
            let token = match require(&f.token, "--token (xoxp-… or xoxb-…)") {
                Ok(v) => v,
                Err(c) => return Ok(c),
            };
            let resp: Value = post_form("https://slack.com/api/auth.test", &token)?;
            if !resp["ok"].as_bool().unwrap_or(false) {
                return connect_err(&format!("Slack rejected the token: {}", resp["error"]));
            }
            json!({"token": token, "team": resp["team"], "user": resp["user"], "url": resp["url"]})
        }
        "github" => {
            let token = match require(&f.token, "--token (github_pat_… or ghp_…)") {
                Ok(v) => v,
                Err(c) => return Ok(c),
            };
            let resp = get_json(
                "https://api.github.com/user",
                &[
                    ("Authorization", &format!("Bearer {token}")),
                    ("User-Agent", "mari"),
                ],
            )?;
            let login = resp["login"]
                .as_str()
                .ok_or_else(|| anyhow!("GitHub rejected the token"))?;
            json!({"token": token, "login": login})
        }
        "discord" => {
            let token = match require(&f.token, "--token (bot token)") {
                Ok(v) => v,
                Err(c) => return Ok(c),
            };
            let resp = get_json(
                "https://discord.com/api/v10/users/@me",
                &[("Authorization", &format!("Bot {token}"))],
            )?;
            json!({"token": token, "name": resp["username"], "id": resp["id"]})
        }
        "linear" => {
            let token = match require(&f.token, "--token (personal API key)") {
                Ok(v) => v,
                Err(c) => return Ok(c),
            };
            let resp: Value = ureq::post("https://api.linear.app/graphql")
                .set("Authorization", &token)
                .set("Content-Type", "application/json")
                .send_json(json!({"query": "{ viewer { name } }"}))
                .map_err(|e| anyhow!("Linear connect error: {e}"))?
                .into_json()?;
            json!({"token": token, "name": resp["data"]["viewer"]["name"]})
        }
        "hubspot" => {
            let token = match require(&f.token, "--token (pat-…)") {
                Ok(v) => v,
                Err(c) => return Ok(c),
            };
            let resp = get_json(
                "https://api.hubapi.com/account-info/v3/details",
                &[("Authorization", &format!("Bearer {token}"))],
            )?;
            json!({"token": token, "portal_id": resp["portalId"]})
        }
        "zendesk" => {
            let sub = match require(&f.subdomain, "--subdomain") {
                Ok(v) => v,
                Err(c) => return Ok(c),
            };
            let email = match require(&f.email, "--email") {
                Ok(v) => v,
                Err(c) => return Ok(c),
            };
            let token = match require(&f.token, "--token (API token)") {
                Ok(v) => v,
                Err(c) => return Ok(c),
            };
            let basic = base64(&format!("{email}/token:{token}"));
            let resp = get_json(
                &format!("https://{sub}.zendesk.com/api/v2/users/me.json"),
                &[("Authorization", &format!("Basic {basic}"))],
            )?;
            if resp["user"]["id"].is_null() {
                return connect_err("Zendesk rejected the credential");
            }
            json!({"subdomain": sub, "email": email, "token": token, "name": resp["user"]["name"]})
        }
        "confluence" | "jira" => {
            let url = match require(&f.url, "--url") {
                Ok(v) => v.trim_end_matches('/').to_string(),
                Err(c) => return Ok(c),
            };
            let token = match require(&f.token, "--token") {
                Ok(v) => v,
                Err(c) => return Ok(c),
            };
            // Method inferred from presence of --email (SPEC §6.5).
            let (auth_header, method) = match &f.email {
                Some(email) => (
                    format!("Basic {}", base64(&format!("{email}:{token}"))),
                    "cloud",
                ),
                None => (format!("Bearer {token}"), "pat"),
            };
            let probe = if provider == "jira" {
                format!("{url}/rest/api/2/myself")
            } else {
                format!("{url}/rest/api/space?limit=1")
            };
            let resp = get_json(&probe, &[("Authorization", &auth_header)])?;
            let name = resp["displayName"].as_str().unwrap_or("connected");
            json!({"method": method, "url": url, "email": f.email, "token": token, "name": name})
        }
        "salesforce" => {
            let url = match require(&f.url, "--url (instance URL)") {
                Ok(v) => v.trim_end_matches('/').to_string(),
                Err(c) => return Ok(c),
            };
            let token = match require(&f.token, "--token (OAuth access token)") {
                Ok(v) => v,
                Err(c) => return Ok(c),
            };
            get_json(
                &format!("{url}/services/data/"),
                &[("Authorization", &format!("Bearer {token}"))],
            )?;
            json!({"token": token, "url": url, "name": "salesforce"})
        }
        "google" => {
            // Rides the user's gcloud session (SPEC §6.2) — no OAuth client needed.
            let account = std::process::Command::new("gcloud")
                .args(["config", "get-value", "account"])
                .output()
                .map_err(|_| anyhow!("gcloud not found — install the Google Cloud CLI and run `gcloud auth login --enable-gdrive-access`"))?;
            let account = String::from_utf8_lossy(&account.stdout).trim().to_string();
            if account.is_empty() || account == "(unset)" {
                return connect_err(
                    "no gcloud account — run `gcloud auth login --enable-gdrive-access`",
                );
            }
            let ok = std::process::Command::new("gcloud")
                .args(["auth", "print-access-token"])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false);
            if !ok {
                return connect_err("gcloud session has no valid token — run `gcloud auth login --enable-gdrive-access`");
            }
            json!({"method": "gcloud", "account": account})
        }
        "microsoft" => {
            return microsoft_device_code();
        }
        _ => unreachable!(),
    };

    let path = credential_path(provider);
    workspace::write_credential(&path, &cred)?;
    println!(
        "✓ {provider} connected — credential saved to {}",
        path.display()
    );
    Ok(0)
}

fn microsoft_device_code() -> Result<i32> {
    // Device-code flow against the public Azure CLI client (SPEC §6.10):
    // no app registration or admin consent needed.
    const CLIENT_ID: &str = "04b07795-8ddb-461a-bbee-02f9e1bf7b46";
    const SCOPES: &str =
        "offline_access Files.Read.All Mail.Read Chat.Read Sites.Read.All User.Read";
    let dc: Value = ureq::post("https://login.microsoftonline.com/common/oauth2/v2.0/devicecode")
        .send_form(&[("client_id", CLIENT_ID), ("scope", SCOPES)])
        .map_err(|e| anyhow!("device-code request failed: {e}"))?
        .into_json()?;
    let message = dc["message"]
        .as_str()
        .unwrap_or("visit the URL and enter the code");
    println!("{message}");
    let device_code = dc["device_code"].as_str().unwrap_or_default().to_string();
    let interval = dc["interval"].as_u64().unwrap_or(5);
    let expires = dc["expires_in"].as_u64().unwrap_or(900);
    let start = std::time::Instant::now();
    loop {
        std::thread::sleep(std::time::Duration::from_secs(interval));
        if start.elapsed().as_secs() > expires {
            return connect_err("device-code flow timed out");
        }
        let resp = ureq::post("https://login.microsoftonline.com/common/oauth2/v2.0/token")
            .send_form(&[
                ("client_id", CLIENT_ID),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", &device_code),
            ]);
        let body: Value = match resp {
            Ok(r) => r.into_json()?,
            Err(ureq::Error::Status(_, r)) => r.into_json()?,
            Err(e) => return Err(anyhow!("token poll failed: {e}")),
        };
        if body["access_token"].is_string() {
            let cred = json!({
                "method": "device_code",
                "client_id": CLIENT_ID,
                "access_token": body["access_token"],
                "refresh_token": body["refresh_token"],
                "scope": SCOPES,
            });
            let path = credential_path("microsoft");
            workspace::write_credential(&path, &cred)?;
            println!(
                "✓ microsoft connected — credential saved to {}",
                path.display()
            );
            return Ok(0);
        }
        match body["error"].as_str() {
            Some("authorization_pending") | Some("slow_down") => continue,
            Some(e) => return connect_err(&format!("device-code flow failed: {e}")),
            None => return connect_err("device-code flow failed"),
        }
    }
}

pub const SOURCES: &[&str] = &[
    "slack",
    "gdocs",
    "github",
    "git",
    "confluence",
    "jira",
    "zendesk",
    "salesforce",
    "hubspot",
    "microsoft",
    "discord",
    "linear",
    "localfiles",
];

pub fn scope(source: Option<&str>, new_scope: Option<&str>) -> Result<i32> {
    match (source, new_scope) {
        (None, _) => {
            for s in SOURCES {
                println!("{s:<12} {}", workspace::source_scope(s));
            }
            Ok(0)
        }
        (Some(s), None) => {
            if !SOURCES.contains(&s) {
                eprintln!("unknown source: {s}");
                return Ok(2);
            }
            println!("{}", workspace::source_scope(s));
            Ok(0)
        }
        (Some(s), Some(sc)) => {
            if !SOURCES.contains(&s) {
                eprintln!("unknown source: {s}");
                return Ok(2);
            }
            if sc != "global" && sc != "local" {
                eprintln!("scope must be `global` or `local`");
                return Ok(2);
            }
            workspace::set_source_scope(s, sc)?;
            println!("✓ {s} scope = {sc}");
            Ok(0)
        }
    }
}

/// Missing required field → usage error, exit 2 (SPEC §5.1).
fn require(v: &Option<String>, what: &str) -> std::result::Result<String, i32> {
    match v {
        Some(s) => Ok(s.clone()),
        None => {
            eprintln!("missing required flag {what}");
            Err(2)
        }
    }
}

fn connect_err(msg: &str) -> Result<i32> {
    eprintln!("✗ {msg}");
    Ok(1)
}

fn get_json(url: &str, headers: &[(&str, &str)]) -> Result<Value> {
    let mut req = ureq::get(url).timeout(std::time::Duration::from_secs(60));
    for (k, v) in headers {
        req = req.set(k, v);
    }
    match req.call() {
        Ok(r) => Ok(r.into_json()?),
        Err(ureq::Error::Status(code, _)) => Err(anyhow!("connect error: HTTP {code} from {url}")),
        Err(e) => Err(anyhow!("connect error: {e}")),
    }
}

fn post_form(url: &str, token: &str) -> Result<Value> {
    match ureq::post(url)
        .set("Authorization", &format!("Bearer {token}"))
        .timeout(std::time::Duration::from_secs(60))
        .call()
    {
        Ok(r) => Ok(r.into_json()?),
        Err(ureq::Error::Status(code, _)) => Err(anyhow!("connect error: HTTP {code}")),
        Err(e) => Err(anyhow!("connect error: {e}")),
    }
}

pub fn base64(s: &str) -> String {
    // Minimal base64 (standard alphabet, padded) to avoid another dependency.
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let b = s.as_bytes();
    let mut out = String::new();
    for chunk in b.chunks(3) {
        let n = ((chunk[0] as u32) << 16)
            | ((chunk.get(1).copied().unwrap_or(0) as u32) << 8)
            | chunk.get(2).copied().unwrap_or(0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn base64_known_values() {
        assert_eq!(super::base64("ab"), "YWI=");
        assert_eq!(super::base64("a"), "YQ==");
        assert_eq!(super::base64("abc"), "YWJj");
    }
}
