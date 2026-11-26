use aes::cipher::{BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
use axum::http::{HeaderMap, HeaderValue};
use base64::Engine as _;
use hmac::{Hmac, Mac};
use santi_api::webhook::{Adaptor, Error, Event, Outcome, feishu::Feishu, github::Github};
use serde_json::json;
use sha2::{Digest, Sha256};

const SECRET: &str = "test-secret";
const TOKEN: &str = "verification-token-1";
const KEY: &str = "encrypt-key-1";

mod github {
    use super::*;

    #[test]
    fn signature() {
        let body = body();
        let signature = sign(SECRET, body);
        let adaptor = Github::configured(None, None).addressed(Some("AdamSanti"));
        assert!(
            adaptor
                .verify(&headers("issue_comment", Some(&signature)), body, SECRET)
                .is_ok()
        );
        assert!(matches!(
            adaptor.verify(
                &headers("issue_comment", Some(&sign("wrong", body))),
                body,
                SECRET
            ),
            Err(Error::Unauthorized(_))
        ));
        assert!(matches!(
            adaptor.verify(&headers("issue_comment", None), body, SECRET),
            Err(Error::Unauthorized(_))
        ));
    }

    #[test]
    fn normalizes() {
        let event = unwrap(
            Github::configured(None, None)
                .addressed(Some("AdamSanti"))
                .normalize(&headers("issue_comment", None), body(), SECRET, "ops")
                .expect("normalize"),
        );
        assert!(event.scoped);
        assert_eq!(event.label, "github:ops:issue:PerishCode/santi#42");
        assert_eq!(event.delivery.as_deref(), Some("issuecomment-1"));
        assert!(
            event
                .text
                .contains("posted a comment on an Issue assigned to you")
        );
        assert_eq!(
            event.metadata.as_ref().unwrap()["delivery"],
            "issuecomment-1"
        );
        assert!(!event.text.contains("delivery:"));
        assert!(event.text.contains("requested work"));
    }

    #[test]
    fn senders() {
        let headers = headers("issue_comment", None);
        let allowed = unwrap(
            Github::configured(None, Some("SomeHuman"))
                .addressed(Some("AdamSanti"))
                .normalize(&headers, body(), SECRET, "ops")
                .expect("normalize"),
        );
        assert!(allowed.scoped);
        let rejected = unwrap(
            Github::configured(None, Some("someone-else"))
                .addressed(Some("AdamSanti"))
                .normalize(&headers, body(), SECRET, "ops")
                .expect("normalize"),
        );
        assert!(!rejected.scoped);
        let own = unwrap(
            Github::configured(Some("somehuman"), None)
                .addressed(Some("AdamSanti"))
                .normalize(&headers, body(), SECRET, "ops")
                .expect("normalize"),
        );
        assert!(own.authored);
    }

    #[test]
    fn ping() {
        let event = unwrap(
            Github::configured(None, None)
                .addressed(Some("AdamSanti"))
                .normalize(&headers("ping", None), br#"{"zen":"ok"}"#, SECRET, "ops")
                .expect("normalize"),
        );
        assert!(!event.scoped);
    }
}

#[path = "webhook/feishu.rs"]
mod feishu;

fn unwrap(outcome: Outcome) -> Event {
    match outcome {
        Outcome::Event(event) => event,
        Outcome::Reply(value) => panic!("expected event, got {value}"),
    }
}

fn sign(secret: &str, body: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(body);
    format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
}

fn headers(event: &str, signature: Option<&str>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("x-github-event", HeaderValue::from_str(event).unwrap());
    headers.insert(
        "x-github-delivery",
        HeaderValue::from_static("issuecomment-1"),
    );
    if let Some(signature) = signature {
        headers.insert(
            "x-hub-signature-256",
            HeaderValue::from_str(signature).unwrap(),
        );
    }
    headers
}

fn body() -> &'static [u8] {
    br#"{
        "action":"created",
        "repository":{"full_name":"PerishCode/santi"},
        "issue":{"number":42,"title":"title","assignees":[{"login":"AdamSanti"}]},
        "comment":{"body":"requested work","html_url":"https://github.com/PerishCode/santi/issues/42#issuecomment-1"},
        "sender":{"login":"somehuman"}
    }"#
}

fn payload(sender_type: &str, open_id: &str, event_type: &str) -> String {
    format!(
        r#"{{
            "schema":"2.0",
            "header":{{"event_id":"ev_123","event_type":"{event_type}","token":"{TOKEN}"}},
            "event":{{
                "sender":{{"sender_type":"{sender_type}","sender_id":{{"open_id":"{open_id}","user_id":"u_1"}}}},
                "message":{{"chat_id":"oc_chat1","chat_type":"p2p","message_id":"om_msg1","content":"top secret"}}
            }}
        }}"#
    )
}

fn encrypt(plaintext: &str) -> String {
    let key = Sha256::digest(KEY.as_bytes());
    let iv = [7u8; 16];
    let ciphertext = cbc::Encryptor::<aes::Aes256>::new_from_slices(&key, &iv)
        .unwrap()
        .encrypt_padded_vec_mut::<Pkcs7>(plaintext.as_bytes());
    let mut data = iv.to_vec();
    data.extend_from_slice(&ciphertext);
    base64::engine::general_purpose::STANDARD.encode(data)
}
