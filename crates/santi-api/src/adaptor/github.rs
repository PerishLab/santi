use axum::http::HeaderMap;
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::Sha256;

use super::{Adaptor, Error, Event, Outcome, allowed, field};

mod render;
mod scope;

type Hmac256 = Hmac<Sha256>;

pub struct Github {
    login: Option<String>,
    allow: Option<String>,
    recipient: Option<String>,
}

impl Github {
    pub fn configured(login: Option<&str>, allow: Option<&str>) -> Self {
        Self {
            login: normalized(login),
            allow: normalized(allow),
            recipient: None,
        }
    }
}

impl Github {
    pub fn addressed(mut self, recipient: Option<&str>) -> Self {
        self.recipient = normalized(recipient);
        self
    }
}

impl Adaptor for Github {
    fn verify(&self, headers: &HeaderMap, raw: &[u8], secret: &str) -> Result<(), Error> {
        let presented = headers
            .get("X-Hub-Signature-256")
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("sha256="))
            .ok_or_else(|| Error::Unauthorized("missing X-Hub-Signature-256 header".to_string()))?;
        let presented = hex::decode(presented)
            .map_err(|_| Error::Unauthorized("malformed signature hex".to_string()))?;
        let mut mac = Hmac256::new_from_slice(secret.as_bytes())
            .map_err(|error| Error::Unauthorized(error.to_string()))?;
        mac.update(raw);
        mac.verify_slice(&presented)
            .map_err(|_| Error::Unauthorized("signature mismatch".to_string()))
    }

    fn normalize(
        &self,
        headers: &HeaderMap,
        raw: &[u8],
        _: &str,
        name: &str,
    ) -> Result<Outcome, Error> {
        let kind = headers
            .get("X-GitHub-Event")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string();
        let payload: Value = serde_json::from_slice(raw)
            .map_err(|error| Error::Invalid(format!("invalid JSON body: {error}")))?;

        if kind != "issues" && kind != "issue_comment" {
            return Ok(Outcome::Event(Event {
                text: String::new(),
                label: format!("github:{name}:{kind}"),
                metadata: None,
                delivery: None,
                scoped: false,
                authored: false,
            }));
        }

        let repo = field(&payload, &["repository", "full_name"]).unwrap_or_default();
        let number = payload
            .pointer("/issue/number")
            .and_then(Value::as_i64)
            .unwrap_or_default();
        let action = payload
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let url = if kind == "issue_comment" {
            field(&payload, &["comment", "html_url"])
        } else {
            field(&payload, &["issue", "html_url"])
        }
        .unwrap_or_default();
        let delivery = headers
            .get("X-GitHub-Delivery")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_string();
        let replay = (!delivery.is_empty()).then(|| delivery.clone());
        let sender = field(&payload, &["sender", "login"]).unwrap_or_default();
        let authored = [self.login.as_deref(), self.recipient.as_deref()]
            .into_iter()
            .flatten()
            .any(|login| login.eq_ignore_ascii_case(&sender));
        let permitted = allowed(&sender, self.allow.as_deref());
        let relation = if permitted && !authored {
            let recipient = self.recipient.as_deref().ok_or_else(|| Error::Unavailable(
                "Configure the receiving Soul's channels.github account before accepting GitHub Issue events".into()
            ))?;
            scope::related(&payload, &kind, recipient)?
        } else {
            scope::Relation::Unrelated
        };
        let scoped = permitted && relation != scope::Relation::Unrelated;
        if scoped && !scope::topic(&repo, number) {
            return Err(Error::Invalid(
                "GitHub Issue repository/number is unavailable or invalid".into(),
            ));
        }
        let (text, material) = render::rendered(render::Event {
            payload: &payload,
            event: &kind,
            actor: &sender,
            recipient: self.recipient.as_deref(),
            relation,
        });
        let label = format!("github:{name}:issue:{repo}#{number}");
        let metadata = json!({
            "adaptor": "github",
            "webhook_name": name,
            "event_type": kind,
            "action": action,
            "delivery": delivery,
            "repo": repo,
            "issue_number": number,
            "url": url,
            "label": label,
            "actor": sender,
            "recipient": self.recipient,
            "relationship": relation.label(),
            "content": material,
        });

        Ok(Outcome::Event(Event {
            text,
            label,
            metadata: Some(metadata),
            delivery: replay,
            scoped,
            authored,
        }))
    }
}

fn normalized(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}
