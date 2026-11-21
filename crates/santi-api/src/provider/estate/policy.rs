use crate::config::Resolved;
use hmac::{Hmac, Mac};
use santi_core::inference::Route;
use serde_json::json;
use sha2::{Digest, Sha256};

pub(super) fn stamp(
    profile: &Resolved,
    seal: Option<&str>,
    route: &Route,
) -> Result<String, String> {
    let (settings, templates) = match profile {
        Resolved::ChatCompletions(p) => (
            json!([
                p.bytes,
                p.maintenance,
                p.max_tokens,
                p.thinking,
                p.replay == santi_provider::chat::completions::Replay::History,
                p.headers
            ]),
            &p.outgoing,
        ),
        Resolved::Gemini(p) => (
            json!([p.bytes, p.maintenance, p.max_tokens, p.headers]),
            &p.outgoing,
        ),
        Resolved::OpenAiResponses(p) => (
            json!([
                p.bytes,
                p.maintenance,
                p.max_output_tokens,
                p.attempts,
                p.summary,
                p.headers
            ]),
            &p.outgoing,
        ),
    };
    if templates.is_empty() {
        return Ok(format!(
            "public:{:x}",
            Sha256::digest(settings.to_string().as_bytes())
        ));
    }
    let invalid = "custody request_headers require a dedicated 32-byte hex seal";
    let seal = seal.ok_or(invalid)?;
    let secret = match seal.strip_prefix("env://") {
        Some(name) => crate::config::env(name).ok_or(invalid)?,
        None => seal.to_owned(),
    };
    let mut key = [0; 32];
    hex::decode_to_slice(secret.trim(), &mut key).map_err(|_| invalid)?;
    let mut mac = Hmac::<Sha256>::new_from_slice(&key).map_err(|_| invalid)?;
    mac.update(b"santi/custody/transport/v2\0");
    let input = json!({"custody":route.custody,"access":route.access.id,"protocol":route.protocol,"endpoint":route.endpoint,"settings":settings,"templates":templates});
    mac.update(&serde_json::to_vec(&input).map_err(|_| "cannot encode custody policy")?);
    Ok(format!(
        "sealed:{}",
        hex::encode(mac.finalize().into_bytes())
    ))
}
