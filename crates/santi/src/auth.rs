use std::time::Duration;

use anyhow::{Context, Result};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

pub(crate) struct Credentials<'a> {
    pub(crate) endpoint: Option<&'a str>,
    pub(crate) identity: Option<&'a str>,
    pub(crate) username: Option<&'a str>,
    pub(crate) password: Option<&'a str>,
    pub(crate) key: Option<&'a str>,
    pub(crate) cache: Option<&'a std::path::Path>,
}

pub(crate) async fn resolve_edge_bearer(credentials: Credentials<'_>) -> Result<Option<String>> {
    fn present(value: Option<&str>) -> Option<&str> {
        value.filter(|value| !value.is_empty())
    }

    if let Some(url) = present(credentials.endpoint)
        && let Some(cid) = present(credentials.identity)
        && let Some(user) = present(credentials.username)
        && let Some(pw) = present(credentials.password)
    {
        let path = credentials
            .cache
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| edge_token_cache_path(url, cid, user));
        let edge = Edge {
            url,
            identity: cid,
            username: user,
            password: pw,
        };
        return Ok(Some(edge_jwt_cached(&path, edge).await?));
    }
    Ok(present(credentials.key).map(str::to_string))
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

struct Edge<'a> {
    url: &'a str,
    identity: &'a str,
    username: &'a str,
    password: &'a str,
}

async fn edge_jwt_cached(path: &std::path::Path, edge: Edge<'_>) -> Result<String> {
    let now = now_secs();
    if let Ok(bytes) = std::fs::read(path)
        && let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes)
        && let Some(token) = v.get("access_token").and_then(|t| t.as_str())
        && v.get("expires_at").and_then(|t| t.as_u64()).unwrap_or(0) > now + 60
    {
        return Ok(token.to_string());
    }
    let (access_token, expires_in) =
        fetch_edge_jwt(edge.url, edge.identity, edge.username, edge.password).await?;
    let v = serde_json::json!({ "access_token": access_token, "expires_at": now + expires_in });
    let _ = write_token_cache(path, &v);
    Ok(access_token)
}

async fn fetch_edge_jwt(
    token_url: &str,
    client_id: &str,
    username: &str,
    password: &str,
) -> Result<(String, u64)> {
    let client = reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .context("build token client")?;
    let body = form_urlencode(&[
        ("grant_type", "client_credentials"),
        ("client_id", client_id),
        ("username", username),
        ("password", password),
        ("scope", "openid"),
    ]);
    let response = client
        .post(token_url)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await
        .with_context(|| format!("POST {token_url}"))?;
    let status = response.status();
    let text = response.text().await.context("read token response")?;
    if !status.is_success() {
        let detail: String = text.chars().take(200).collect();
        anyhow::bail!("edge token endpoint {token_url} -> {status}: {detail}");
    }
    let value: serde_json::Value = serde_json::from_str(&text).context("parse token response")?;
    let access_token = value
        .get("access_token")
        .and_then(|t| t.as_str())
        .context("token response missing access_token")?
        .to_string();
    let expires_in = value
        .get("expires_in")
        .and_then(|t| t.as_u64())
        .unwrap_or(3600);
    Ok((access_token, expires_in))
}

fn edge_token_cache_path(token_url: &str, client_id: &str, username: &str) -> std::path::PathBuf {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    token_url.hash(&mut hasher);
    client_id.hash(&mut hasher);
    username.hash(&mut hasher);
    let key = format!("{:016x}", hasher.finish());
    let dir = crate::config::shelter();
    dir.join(format!("edge-jwt-{key}.json"))
}

pub fn form_urlencode(pairs: &[(&str, &str)]) -> String {
    fn enc(input: &str) -> String {
        let mut out = String::with_capacity(input.len());
        for byte in input.bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                    out.push(byte as char)
                }
                b' ' => out.push('+'),
                _ => out.push_str(&format!("%{byte:02X}")),
            }
        }
        out
    }
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", enc(k), enc(v)))
        .collect::<Vec<_>>()
        .join("&")
}

fn write_token_cache(path: &std::path::Path, value: &serde_json::Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_vec(value)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Credentials, resolve_edge_bearer};

    #[tokio::test]
    async fn sentinels() {
        let bearer = resolve_edge_bearer(Credentials {
            endpoint: Some(""),
            identity: Some(""),
            username: Some(""),
            password: Some(""),
            key: Some("resolved bearer"),
            cache: None,
        })
        .await
        .expect("empty sentinels must not trigger edge auth");

        assert_eq!(bearer.as_deref(), Some("resolved bearer"));
    }
}
