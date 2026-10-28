use super::{TIMEOUT, Target};

pub(super) async fn request(target: Target<'_>, turn: String) -> (String, Result<bool, String>) {
    let result = stop(target, &turn).await;
    (turn, result)
}

async fn stop(target: Target<'_>, turn: &str) -> Result<bool, String> {
    let url = format!("{}/api/v1/turns/{turn}/stop", target.base);
    let response = target
        .client
        .post(&url)
        .timeout(TIMEOUT)
        .send()
        .await
        .map_err(|error| format!("interrupt for turn {turn} is unconfirmed: {error}"))?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!(
            "interrupt for turn {turn} returned status {status}"
        ));
    }
    let value: serde_json::Value = response
        .json()
        .await
        .map_err(|error| format!("interrupt response for turn {turn} is unreadable: {error}"))?;
    if value
        .pointer("/turn/id")
        .and_then(serde_json::Value::as_str)
        != Some(turn)
    {
        return Err(format!(
            "interrupt response identity did not match turn {turn}"
        ));
    }
    value
        .get("accepted")
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| format!("interrupt response for turn {turn} did not classify acceptance"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn acceptance() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            for body in [
                r#"{"turn":{"id":"other"},"accepted":true}"#,
                r#"{"turn":{"id":"current"}}"#,
                r#"{"turn":{"id":"current"},"accepted":false}"#,
                r#"{"turn":{"id":"current"},"accepted":true}"#,
            ] {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = [0; 4096];
                let length = stream.read(&mut request).await.unwrap();
                assert!(
                    String::from_utf8_lossy(&request[..length])
                        .starts_with("POST /api/v1/turns/current/stop ")
                );
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            }
        });
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        let target = Target::tui(&client, &base, "strand");
        assert!(stop(target, "current").await.is_err());
        assert!(stop(target, "current").await.is_err());
        assert!(!stop(target, "current").await.unwrap());
        assert!(stop(target, "current").await.unwrap());
        server.await.unwrap();
    }
}
