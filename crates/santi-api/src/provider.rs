use std::collections::BTreeMap;
use std::sync::Arc;

use santi_core::inference::{Declaration, Effective, Origin};
use santi_provider::{Provider, chat::completions, openai};

use crate::config::{ChatCompletions, OpenAiResponses, Profile, Providers, Resolved};

pub struct Catalog {
    pub profiles: &'static BTreeMap<String, Profile>,
}

impl santi_core::provider::Resolver for Catalog {
    fn resolve(&self, layers: &[(&Declaration, Origin)]) -> Result<Effective, String> {
        Effective::resolve(layers, &Providers(self.profiles))
    }

    fn build(&self, effective: &Effective) -> Result<Arc<dyn Provider>, String> {
        let profile = self.profiles.get(&effective.provider).ok_or_else(|| {
            format!(
                "provider {} is not defined in the config",
                effective.provider
            )
        })?;
        let resolved = profile.resolve(&effective.provider)?.rebase(effective)?;
        Ok(build(resolved))
    }
}

pub fn build(config: Resolved) -> Arc<dyn Provider> {
    match config {
        Resolved::OpenAiResponses(config) => responses(config),
        Resolved::ChatCompletions(config) => completions(config),
    }
}

fn responses(config: OpenAiResponses) -> Arc<dyn Provider> {
    Arc::new(openai::OpenAI::new(openai::Config {
        key: config.api_key,
        model: config.model,
        url: config.base_url,
        effort: config.reasoning_effort,
        summary: config.summary,
        ceiling: config.max_output_tokens,
        bytes: Some(config.bytes),
        maintenance: Some(config.maintenance),
        attempts: config.attempts,
        headers: config.headers,
    }))
}

fn completions(config: ChatCompletions) -> Arc<dyn Provider> {
    Arc::new(completions::Chat::new(completions::Config {
        provider: config.provider,
        key: config.api_key,
        model: config.model,
        url: config.base_url,
        thinking: config.thinking,
        replay: config.replay,
        effort: config.reasoning_effort,
        ceiling: config.max_tokens,
        bytes: Some(config.bytes),
        maintenance: Some(config.maintenance),
        headers: config.headers,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use santi_core::provider::Resolver;

    fn profiles() -> &'static BTreeMap<String, Profile> {
        let parsed: BTreeMap<String, Profile> = toml::from_str(
            r#"
[chat]
kind = "chat_completions"
api_key = "fixture"
model = "default"
base_url = "http://provider.test"
reasoning_effort = "low"
bytes = 1000
maintenance_bytes = 1500
"#,
        )
        .unwrap();
        Box::leak(Box::new(parsed))
    }

    #[test]
    fn builds() {
        let resolver = Catalog {
            profiles: profiles(),
        };
        let declaration = Declaration {
            provider: Some("chat".into()),
            model: Some("declared".into()),
            reasoning_effort: Some("high".into()),
        };
        let effective = resolver
            .resolve(&[(&declaration, Origin::Service)])
            .unwrap();
        assert_eq!(effective.provider, "chat");
        assert_eq!(effective.model, "declared");
        assert_eq!(effective.reasoning_effort.as_deref(), Some("high"));

        let provider = resolver.build(&effective).unwrap();
        let metadata = provider.metadata();
        assert_eq!(&*metadata.provider, "chat");
        assert_eq!(metadata.model, "declared");
        let budget = metadata.budget.expect("profile budget");
        assert_eq!(budget.bytes, 1000);
        assert_eq!(budget.maintenance, 1500);
        assert_eq!(budget.source, "provider_config");
    }

    #[test]
    fn rejects() {
        let resolver = Catalog {
            profiles: profiles(),
        };
        let declaration = Declaration {
            provider: Some("missing".into()),
            ..Default::default()
        };
        let error = resolver
            .resolve(&[(&declaration, Origin::Service)])
            .unwrap_err();

        assert!(error.contains("missing"), "{error}");
    }

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
[chat]
kind = "chat_completions"
api_key = "fixture-key"
model = "default"
base_url = "http://{address}"
reasoning_effort = "low"
bytes = 1000
maintenance_bytes = 1500
"#
        );
        let profiles: BTreeMap<String, Profile> = toml::from_str(&raw).unwrap();
        let profiles = Box::leak(Box::new(profiles));
        let resolver = Catalog { profiles };
        let declaration = Declaration {
            provider: Some("chat".into()),
            model: Some("declared".into()),
            reasoning_effort: Some("high".into()),
        };
        let effective = resolver
            .resolve(&[(&declaration, Origin::Service)])
            .unwrap();
        let provider = resolver.build(&effective).unwrap();
        let request = santi_provider::Request {
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
        assert!(raw.contains("\"model\":\"declared\""), "{raw}");
        assert!(raw.contains("\"reasoning_effort\":\"high\""), "{raw}");
    }
}
