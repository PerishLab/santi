use anyhow::Result;

use super::*;

pub(super) fn compact_capsule_body(capsule: Capsule<'_>) -> Result<serde_json::Value> {
    let Capsule {
        first,
        last,
        from,
        to,
        summary,
        file: summary_file,
        source,
        reason,
        risk,
        queryability,
        preview: dry,
        soul,
    } = capsule;
    let summary = match summary_file {
        Some(path) => read_summary_file(&path)?,
        None => summary.expect("clap requires summary or summary_file"),
    };
    let mut body = serde_json::json!({
        "first": first,
        "last": last,
        "from": from,
        "to": to,
        "summary": summary,
        "capsule": {
            "source": source,
            "reason": reason,
            "risk": risk,
            "queryability": queryability,
        },
        "dry": dry,
    });
    if let Some(soul) = soul {
        body["soul"] = serde_json::Value::from(soul);
    }
    Ok(body)
}

pub(super) fn urlencoding_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

pub(super) fn build_client(bearer: Option<&str>) -> Result<reqwest::Client> {
    let mut builder = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never());
    if let Some(token) = bearer {
        let mut headers = reqwest::header::HeaderMap::new();
        let mut value = reqwest::header::HeaderValue::from_str(&format!("Bearer {token}"))
            .context("invalid bearer token")?;
        value.set_sensitive(true);
        headers.insert(reqwest::header::AUTHORIZATION, value);
        builder = builder.default_headers(headers);
    }
    builder.build().context("build http client")
}

pub(super) struct Http<'a> {
    pub(super) client: &'a reqwest::Client,
}

impl Http<'_> {
    async fn send(&self, request: reqwest::RequestBuilder, label: String) -> Result<()> {
        let response = request.timeout(TIMEOUT).send().await;
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                transport(&error);
                return Err(anyhow::Error::new(error)).with_context(|| label);
            }
        };
        print_json(response).await
    }

    #[locus::trace(with = plumb::observation::view())]
    pub(super) async fn get(&self, url: &str) -> Result<()> {
        self.send(self.client.get(url), format!("GET {url}")).await
    }

    #[locus::trace(with = plumb::observation::view())]
    pub(super) async fn post(&self, url: &str, body: Option<serde_json::Value>) -> Result<()> {
        let mut request = self.client.post(url);
        if let Some(body) = body {
            request = request.json(&body);
        }
        self.send(request, format!("POST {url}")).await
    }

    #[locus::trace(with = plumb::observation::view())]
    pub(super) async fn patch(&self, url: &str, body: serde_json::Value) -> Result<()> {
        self.send(self.client.patch(url).json(&body), format!("PATCH {url}"))
            .await
    }

    #[locus::trace(with = plumb::observation::view())]
    pub(super) async fn delete(&self, url: &str) -> Result<()> {
        self.send(self.client.delete(url), format!("DELETE {url}"))
            .await
    }

    pub(super) async fn owned(&self, url: &str, soul: &str) -> Result<()> {
        self.send(
            self.client.get(url).header("x-santi-soul-id", soul),
            format!("GET {url}"),
        )
        .await
    }

    pub(super) async fn owner(&self, url: &str, soul: &str, body: serde_json::Value) -> Result<()> {
        self.send(
            self.client
                .post(url)
                .header("x-santi-soul-id", soul)
                .json(&body),
            format!("POST {url}"),
        )
        .await
    }

    pub(super) async fn act(&self, url: &str, soul: &str) -> Result<()> {
        self.send(
            self.client.post(url).header("x-santi-soul-id", soul),
            format!("POST {url}"),
        )
        .await
    }

    pub(super) async fn spawn(
        &self,
        url: &str,
        capability: &str,
        body: serde_json::Value,
    ) -> Result<()> {
        let response = self
            .client
            .post(url)
            .header("x-santi-job-capability", capability)
            .json(&body)
            .timeout(TIMEOUT)
            .send()
            .await;
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                transport(&error);
                return Err(anyhow::Error::new(error)).with_context(|| format!("POST {url}"));
            }
        };
        if !response.status().is_success() {
            return refuse(response).await;
        }
        print_json(response).await
    }

    pub(super) async fn follow(&self, url: &str, format: WatchFormat) -> Result<()> {
        let response = self.client.get(url).send().await;
        let response = match response {
            Ok(response) => response,
            Err(error) => {
                transport(&error);
                return Err(anyhow::Error::new(error)).with_context(|| format!("GET {url}"));
            }
        };
        let status = response.status();
        if !status.is_success() {
            unexpected();
            anyhow::bail!("request failed with status {status}");
        }
        let mut stream = response.bytes_stream();
        let mut stdout = std::io::stdout();
        match format {
            WatchFormat::Raw => {
                while let Some(chunk) = stream.next().await {
                    let chunk = read(chunk).context("read event stream")?;
                    stdout.write_all(&chunk).context("write event chunk")?;
                    stdout.flush().ok();
                }
            }
            WatchFormat::Filtered => {
                let mut buffer = Vec::new();
                while let Some((event, data)) = next_sse_frame(&mut stream, &mut buffer).await? {
                    print_watch_line(&mut stdout, &event, &data);
                }
            }
        }
        Ok(())
    }
}

pub(super) async fn print_json(response: reqwest::Response) -> Result<()> {
    let status = response.status();
    let text = read(response.text().await).context("read response body")?;
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(value) => println!("{}", serde_json::to_string_pretty(&value)?),
        Err(_) => println!("{text}"),
    }
    if !status.is_success() {
        body(&text);
        anyhow::bail!("request failed with status {status}");
    }
    Ok(())
}

pub(super) fn strand_send_body(text: String, soul: Option<&str>) -> serde_json::Value {
    let mut content = serde_json::json!({
        "content": [{ "type": "text", "text": text }]
    });
    if let Some(soul) = soul {
        content["soul"] = serde_json::Value::from(soul);
    }
    content
}

pub(super) fn print_watch_line(stdout: &mut impl std::io::Write, event: &str, data: &str) {
    if let Some(line) = render_watch_event(event, data) {
        writeln!(stdout, "{line}").ok();
        stdout.flush().ok();
    }
}

async fn refuse(mut response: reqwest::Response) -> Result<()> {
    const LIMIT: usize = 4096;
    const CWD: &str = "job cwd must be a workspace URI: use soul://, soul://<path>, strand://, or strand://<path> with no parent traversal; omit cwd to use the runtime execution directory (not the creating shell cwd)";
    const CONFLICT: &str = "job capability conflicts with its accepted request";
    let status = response.status();
    if !matches!(
        status,
        reqwest::StatusCode::BAD_REQUEST | reqwest::StatusCode::CONFLICT
    ) {
        unexpected();
        anyhow::bail!("job creation failed with status {status}");
    }
    let mut payload = Vec::new();
    while let Some(chunk) = next(&mut response).await {
        if chunk.len() > LIMIT - payload.len() {
            break;
        }
        payload.extend_from_slice(&chunk);
    }
    body(&String::from_utf8_lossy(&payload));
    let value =
        serde_json::from_slice::<serde_json::Value>(&payload).unwrap_or(serde_json::Value::Null);
    match (
        status,
        value.get("message").and_then(serde_json::Value::as_str),
    ) {
        (reqwest::StatusCode::BAD_REQUEST, Some(CWD)) => {
            anyhow::bail!("job creation refused with status {status}: {CWD}");
        }
        (reqwest::StatusCode::CONFLICT, Some(CONFLICT)) => {
            anyhow::bail!(
                "job creation refused with status {status}: {CONFLICT}. Each shell tool call can create one distinct Job; use a new shell tool call for another Job. Identical request retries reuse the original Job."
            );
        }
        _ => {}
    }
    anyhow::bail!("job creation failed with status {status}")
}

pub(super) fn page(url: String, limit: i64, before: Option<&str>) -> Result<String> {
    let mut url = reqwest::Url::parse(&url)?;
    url.query_pairs_mut()
        .append_pair("limit", &limit.to_string());
    if let Some(before) = before {
        url.query_pairs_mut().append_pair("before", before);
    }
    Ok(url.into())
}

async fn next(response: &mut reqwest::Response) -> Option<Vec<u8>> {
    match response.chunk().await {
        Ok(Some(chunk)) => Some(chunk.to_vec()),
        Ok(None) => None,
        Err(error) => {
            transport(&error);
            None
        }
    }
}
