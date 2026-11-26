use axum::http::HeaderMap;
use base64::Engine as _;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::{Adaptor, Error, Event, Outcome, allowed, field};

pub struct Feishu {
    secret: Option<String>,
    allow: Option<String>,
}

impl Feishu {
    pub fn configured(secret: Option<&str>, allow: Option<&str>) -> Self {
        Self {
            secret: normalized(secret),
            allow: normalized(allow),
        }
    }
}

impl Adaptor for Feishu {
    fn verify(&self, headers: &HeaderMap, raw: &[u8], _: &str) -> Result<(), Error> {
        verify(headers, raw, self.secret.as_deref())
    }

    fn normalize(
        &self,
        _: &HeaderMap,
        raw: &[u8],
        secret: &str,
        name: &str,
    ) -> Result<Outcome, Error> {
        let raw: Value = serde_json::from_slice(raw)
            .map_err(|error| Error::Invalid(format!("invalid JSON body: {error}")))?;
        let payload = match raw.get("encrypt").and_then(Value::as_str) {
            Some(ciphertext) => {
                let key = self.secret.as_deref().ok_or_else(|| {
                    Error::Unauthorized(
                        "encrypted payload but webhooks.feishu.encrypt_key is not set".to_string(),
                    )
                })?;
                let plaintext = decrypt(key, ciphertext)?;
                serde_json::from_slice::<Value>(&plaintext)
                    .map_err(|error| Error::Invalid(format!("invalid decrypted JSON: {error}")))?
            }
            None => raw,
        };

        let token = payload
            .get("token")
            .and_then(Value::as_str)
            .or_else(|| payload.pointer("/header/token").and_then(Value::as_str))
            .unwrap_or("");
        if secret.is_empty() || token != secret {
            return Err(Error::Unauthorized(
                "feishu verification token mismatch".to_string(),
            ));
        }

        if payload.get("type").and_then(Value::as_str) == Some("url_verification") {
            let challenge = payload
                .get("challenge")
                .and_then(Value::as_str)
                .unwrap_or("");
            return Ok(Outcome::Reply(json!({ "challenge": challenge })));
        }

        let kind = payload
            .pointer("/header/event_type")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if kind != "im.message.receive_v1" {
            return Ok(Outcome::Event(Event {
                text: String::new(),
                label: format!("feishu:{name}:{kind}"),
                metadata: None,
                delivery: None,
                scoped: false,
                authored: false,
            }));
        }

        let chat = field(&payload, &["event", "message", "chat_id"]).unwrap_or_default();
        let scope = field(&payload, &["event", "message", "chat_type"]).unwrap_or_default();
        let message = field(&payload, &["event", "message", "message_id"]).unwrap_or_default();
        let id = field(&payload, &["header", "event_id"]).unwrap_or_default();
        let role = field(&payload, &["event", "sender", "sender_type"]).unwrap_or_default();
        let open =
            field(&payload, &["event", "sender", "sender_id", "open_id"]).unwrap_or_default();
        let user =
            field(&payload, &["event", "sender", "sender_id", "user_id"]).unwrap_or_default();
        let permitted =
            allowed(&open, self.allow.as_deref()) || allowed(&user, self.allow.as_deref());
        let scoped = role == "user" && permitted;
        if scoped {
            identifier(&chat, "oc_")?;
            identifier(&message, "om_")?;
            identifier(&id, "")?;
        }
        let fingerprint = hex::encode(Sha256::digest(
            serde_json::to_vec(&payload).map_err(|e| Error::Invalid(e.to_string()))?,
        ));
        let text = format!(
            "[feishu] im.message.receive_v1 in chat {chat} ({scope})\nmessage: {message}\nevent: {id}"
        );
        let label = format!("feishu:{name}:chat:{chat}");
        let metadata = json!({
            "adaptor": "feishu",
            "webhook_name": name,
            "event_type": kind,
            "event_id": id,
            "fingerprint": fingerprint,
            "chat_id": chat,
            "chat_type": scope,
            "message": message,
            "sender_type": role,
            "label": label,
        });

        Ok(Outcome::Event(Event {
            text,
            label,
            metadata: Some(metadata),
            delivery: Some(id),
            scoped,
            authored: false,
        }))
    }
}

fn verify(headers: &HeaderMap, raw: &[u8], secret: Option<&str>) -> Result<(), Error> {
    let presented = headers
        .get("X-Lark-Signature")
        .and_then(|value| value.to_str().ok());
    let Some(presented) = presented else {
        return Ok(());
    };
    let key = secret.ok_or_else(|| {
        Error::Unauthorized("signed payload but webhooks.feishu.encrypt_key is not set".to_string())
    })?;
    let timestamp = headers
        .get("X-Lark-Request-Timestamp")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            Error::Unauthorized("missing X-Lark-Request-Timestamp header".to_string())
        })?;
    let nonce = headers
        .get("X-Lark-Request-Nonce")
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| Error::Unauthorized("missing X-Lark-Request-Nonce header".to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(timestamp.as_bytes());
    hasher.update(nonce.as_bytes());
    hasher.update(key.as_bytes());
    hasher.update(raw);
    let expected = hex::encode(hasher.finalize());
    if expected == presented.trim() {
        Ok(())
    } else {
        Err(Error::Unauthorized("feishu signature mismatch".to_string()))
    }
}

fn decrypt(encrypt_key: &str, ciphertext_b64: &str) -> Result<Vec<u8>, Error> {
    use aes::cipher::{BlockDecryptMut, KeyIvInit, block_padding::Pkcs7};
    let data = base64::engine::general_purpose::STANDARD
        .decode(ciphertext_b64)
        .map_err(|_| Error::Invalid("malformed encrypt field base64".to_string()))?;
    if data.len() <= 16 {
        return Err(Error::Invalid("encrypted payload too short".to_string()));
    }
    let key = Sha256::digest(encrypt_key.as_bytes());
    let (iv, ciphertext) = data.split_at(16);
    cbc::Decryptor::<aes::Aes256>::new_from_slices(&key, iv)
        .map_err(|error| Error::Unauthorized(error.to_string()))?
        .decrypt_padded_vec_mut::<Pkcs7>(ciphertext)
        .map_err(|_| {
            Error::Unauthorized("feishu decryption failed (wrong encrypt key?)".to_string())
        })
}

fn normalized(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn identifier(value: &str, prefix: &str) -> Result<(), Error> {
    if !value.starts_with(prefix) || value.len() <= prefix.len() || value.len() > 160 {
        return Err(Error::Invalid("invalid Feishu event identity".into()));
    }
    if !value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(Error::Invalid("invalid Feishu event identity".into()));
    }
    Ok(())
}
