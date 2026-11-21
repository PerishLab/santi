use std::collections::BTreeMap;
use std::sync::Arc;

use santi_core::inference::{Declaration, Effective, Origin};
use santi_provider::{Provider, chat::completions, openai};

use crate::config::{ChatCompletions, OpenAiResponses, Profile, Providers, Resolved};

mod estate;

pub struct Catalog<'a> {
    pub custody: &'a BTreeMap<String, Profile>,
    pub profiles: &'a BTreeMap<String, Profile>,
}

impl santi_core::provider::Resolver for Catalog<'_> {
    fn resolve(&self, layers: &[(&Declaration, Origin)]) -> Result<Effective, String> {
        Effective::resolve(layers, &Providers(self.profiles))
    }

    fn selecting(
        &self,
        layers: &[(&Declaration, Origin)],
        rows: &[santi_core::sources::Record],
    ) -> Result<Effective, String> {
        if self.custody.is_empty() {
            return self.resolve(layers);
        }
        self.select(layers, rows)
    }

    fn build(&self, effective: &Effective) -> Result<Arc<dyn Provider>, String> {
        build(self.configured(effective)?)
    }
}

impl Catalog<'_> {
    pub(crate) fn configured(&self, effective: &Effective) -> Result<Resolved, String> {
        if let Some(route) = &effective.source.route {
            let resolved = self.bound(route)?;
            return resolved.rebase(effective);
        }
        let profile = self.profiles.get(&effective.provider).ok_or_else(|| {
            format!(
                "provider {} is not defined in the config",
                effective.provider
            )
        })?;
        profile.resolve(&effective.provider)?.rebase(effective)
    }
}

pub fn build(config: Resolved) -> Result<Arc<dyn Provider>, String> {
    match config {
        Resolved::OpenAiResponses(config) => responses(config),
        Resolved::ChatCompletions(config) => completions(config),
        Resolved::Gemini(config) => Ok(Arc::new(santi_provider::gemini::Gemini::new(
            santi_provider::gemini::Config {
                outgoing: crate::config::headers::compile(&config.outgoing)?,
                provider: config.provider,
                key: config.api_key,
                model: config.model,
                url: config.base_url,
                effort: config.reasoning_effort,
                ceiling: config.max_tokens,
                bytes: config.bytes,
                maintenance: config.maintenance,
                headers: config.headers,
            },
        )?)),
    }
}

fn responses(config: OpenAiResponses) -> Result<Arc<dyn Provider>, String> {
    Ok(Arc::new(openai::OpenAI::new(openai::Config {
        outgoing: crate::config::headers::compile(&config.outgoing)?,
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
    })))
}

fn completions(config: ChatCompletions) -> Result<Arc<dyn Provider>, String> {
    Ok(Arc::new(completions::Chat::new(completions::Config {
        outgoing: crate::config::headers::compile(&config.outgoing)?,
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
    })))
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
            custody: Box::leak(Box::default()),
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
            custody: Box::leak(Box::default()),
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
[chat.request_headers]
User-Agent = "{{{{meta.user_agent}}}}"
x-session = "{{{{strand.id}}}}"
"#
        );
        let profiles: BTreeMap<String, Profile> = toml::from_str(&raw).unwrap();
        let profiles = Box::leak(Box::new(profiles));
        let resolver = Catalog {
            profiles,
            custody: Box::leak(Box::default()),
        };
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
        assert!(raw.contains("\"model\":\"declared\""), "{raw}");
        assert!(raw.contains("\"reasoning_effort\":\"high\""), "{raw}");
    }
}
