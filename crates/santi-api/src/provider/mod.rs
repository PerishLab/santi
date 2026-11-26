use std::collections::BTreeMap;
use std::sync::Arc;

use santi_core::inference::{Declaration, Effective, Origin};
use santi_provider::{Provider, chat::completions, openai};

use crate::config::{Completions, Profile, Resolved, Responses};

mod estate;

pub struct Catalog<'a> {
    pub custody: &'a BTreeMap<String, Profile>,
}

impl santi_core::provider::Resolver for Catalog<'_> {
    fn resolve(&self, layers: &[(&Declaration, Origin)]) -> Result<Effective, String> {
        self.select(layers, &[])
    }

    fn selecting(
        &self,
        layers: &[(&Declaration, Origin)],
        rows: &[santi_core::sources::Record],
    ) -> Result<Effective, String> {
        self.select(layers, rows)
    }

    fn build(&self, effective: &Effective) -> Result<Arc<dyn Provider>, String> {
        build(self.configured(effective)?)
    }
}

impl Catalog<'_> {
    pub(crate) fn configured(&self, effective: &Effective) -> Result<Resolved, String> {
        let route = effective
            .source
            .route
            .as_ref()
            .ok_or("admitted turn has no recorded transport")?;
        self.bound(route, &effective.model)?.rebase(effective)
    }
}

pub fn build(config: Resolved) -> Result<Arc<dyn Provider>, String> {
    match config {
        Resolved::Responses(config) => responses(config),
        Resolved::Completions(config) => completions(config),
        Resolved::Gemini(config) => Ok(Arc::new(santi_provider::gemini::Gemini::new(
            santi_provider::gemini::Config {
                outgoing: crate::config::headers::compile(&config.outgoing)?,
                provider: config.provider,
                key: config.key,
                model: config.model,
                url: config.endpoint,
                effort: config.effort,
                ceiling: config.tokens,
                bytes: config.bytes,
                maintenance: config.maintenance,
                headers: config.headers,
            },
        )?)),
    }
}

fn responses(config: Responses) -> Result<Arc<dyn Provider>, String> {
    Ok(Arc::new(openai::Openai::new(openai::Config {
        outgoing: crate::config::headers::compile(&config.outgoing)?,
        key: config.key,
        model: config.model,
        url: config.endpoint,
        effort: config.effort,
        summary: config.summary,
        ceiling: config.tokens,
        bytes: Some(config.bytes),
        maintenance: Some(config.maintenance),
        attempts: config.attempts,
        headers: config.headers,
    })))
}

fn completions(config: Completions) -> Result<Arc<dyn Provider>, String> {
    Ok(Arc::new(completions::Chat::new(completions::Config {
        outgoing: crate::config::headers::compile(&config.outgoing)?,
        provider: config.provider,
        key: config.key,
        model: config.model,
        url: config.endpoint,
        thinking: config.thinking,
        replay: config.replay,
        effort: config.effort,
        ceiling: config.tokens,
        bytes: Some(config.bytes),
        maintenance: Some(config.maintenance),
        headers: config.headers,
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use santi_core::provider::Resolver;

    fn boundary(raw: &[u8]) -> Option<usize> {
        raw.windows(4).position(|part| part == b"\r\n\r\n")
    }

    fn length(raw: &[u8], head: usize) -> usize {
        String::from_utf8_lossy(&raw[..head])
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or(0)
    }

    #[tokio::test]
    async fn effort() {
        use futures_util::StreamExt;
        use std::time::Duration;
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;
        use tokio::sync::oneshot;

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (sent, received) = oneshot::channel();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut raw = Vec::new();
            let mut chunk = [0u8; 1024];
            loop {
                let read = socket.read(&mut chunk).await.unwrap();
                if read == 0 {
                    break;
                }
                raw.extend_from_slice(&chunk[..read]);
                let Some(head) = boundary(&raw) else {
                    continue;
                };
                if raw.len() >= head + 4 + length(&raw, head) {
                    break;
                }
            }
            let _ = sent.send(String::from_utf8_lossy(&raw).to_string());
            let body = "data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\ndata: [DONE]\n\n";
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });

        let raw = format!(
            r#"
[operator]
kind = "chat_completions"
api_key = "fixture-key"
base_url = "http://{address}"
reasoning_effort = "low"
bytes = 1000
maintenance_bytes = 1500
seal = "dededededededededededededededededededededededededededededededede"
[operator.request_headers]
User-Agent = "{{{{meta.user_agent}}}}"
x-session = "{{{{strand.id}}}}"
"#
        );
        use santi_core::sources::{Definition, Protocol};
        let custody = toml::from_str(&raw).unwrap();
        let resolver = Catalog { custody: &custody };
        let mut rows = estate::tests::graph();
        let Definition::Access {
            protocol, endpoint, ..
        } = &mut rows[3].definition
        else {
            unreachable!()
        };
        *protocol = Protocol::Chat;
        *endpoint = format!("http://{address}");
        let declaration = Declaration {
            scenario: Some("gateway/family/review".into()),
            effort: Some("high".into()),
        };
        let effective = resolver
            .selecting(&[(&declaration, Origin::Service)], &rows)
            .unwrap();
        let provider = resolver.build(&effective).unwrap();
        let metadata = provider.metadata();
        let budget = metadata.budget.unwrap();
        assert_eq!(metadata.model, "offered");
        assert_eq!((budget.bytes, budget.maintenance), (1000, 1500));
        assert_eq!(budget.source, "provider_config");
        let request = santi_provider::Request {
            strand: Some("ss_factory".into()),
            model: effective.model.clone(),
            instructions: None,
            input: vec![santi_provider::Item::Message {
                role: "user".into(),
                content: "hello".into(),
            }],
            tools: None,
            previous: None,
        };
        let mut stream = provider.stream(request).await.unwrap();
        loop {
            let next = tokio::time::timeout(Duration::from_secs(5), stream.next())
                .await
                .expect("provider response timed out");
            if next.is_none() {
                break;
            }
        }
        let raw = received.await.unwrap();
        let head = raw.to_ascii_lowercase();
        let agent = plumb::agent!("SANTI", "santi-api").unwrap();
        assert!(head.contains(&format!("user-agent: {}\r\n", agent.value())));
        assert!(head.contains("x-session: ss_factory\r\n"));
        assert!(raw.contains("\"model\":\"offered\""), "{raw}");
        assert!(raw.contains("\"reasoning_effort\":\"high\""), "{raw}");
    }
}
