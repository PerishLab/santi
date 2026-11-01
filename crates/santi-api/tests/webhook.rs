use aes::cipher::{BlockEncryptMut, KeyIvInit, block_padding::Pkcs7};
use axum::http::{HeaderMap, HeaderValue};
use base64::Engine as _;
use hmac::{Hmac, Mac};
use santi_api::webhook::{
    NormalizedEvent, WebhookAdaptor, WebhookError, WebhookOutcome, feishu::FeishuAdaptor,
    github::GithubAdaptor,
};
use serde_json::json;
use sha2::{Digest, Sha256};

const SECRET: &str = "test-secret";
const FEISHU_TOKEN: &str = "verification-token-1";
const FEISHU_KEY: &str = "encrypt-key-1";

mod github {
    use super::*;

    #[test]
    fn signature() {
        let body = github_body();
        let signature = sign(SECRET, body);
        let adaptor = GithubAdaptor::configured(None, None).addressed(Some("AdamSanti"));
        assert!(
            adaptor
                .verify(
                    &github_headers("issue_comment", Some(&signature)),
                    body,
                    SECRET
                )
                .is_ok()
        );
        assert!(matches!(
            adaptor.verify(
                &github_headers("issue_comment", Some(&sign("wrong", body))),
                body,
                SECRET
            ),
            Err(WebhookError::Unauthorized(_))
        ));
        assert!(matches!(
            adaptor.verify(&github_headers("issue_comment", None), body, SECRET),
            Err(WebhookError::Unauthorized(_))
        ));
    }

    #[test]
    fn normalizes() {
        let event = expect_event(
            GithubAdaptor::configured(None, None)
                .addressed(Some("AdamSanti"))
                .normalize(
                    &github_headers("issue_comment", None),
                    github_body(),
                    SECRET,
                    "ops",
                )
                .expect("normalize"),
        );
        assert!(event.in_scope);
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
        let headers = github_headers("issue_comment", None);
        let allowed = expect_event(
            GithubAdaptor::configured(None, Some("SomeHuman"))
                .addressed(Some("AdamSanti"))
                .normalize(&headers, github_body(), SECRET, "ops")
                .expect("normalize"),
        );
        assert!(allowed.in_scope);
        let rejected = expect_event(
            GithubAdaptor::configured(None, Some("someone-else"))
                .addressed(Some("AdamSanti"))
                .normalize(&headers, github_body(), SECRET, "ops")
                .expect("normalize"),
        );
        assert!(!rejected.in_scope);
        let own = expect_event(
            GithubAdaptor::configured(Some("somehuman"), None)
                .addressed(Some("AdamSanti"))
                .normalize(&headers, github_body(), SECRET, "ops")
                .expect("normalize"),
        );
        assert!(own.self_authored);
    }

    #[test]
    fn ping() {
        let event = expect_event(
            GithubAdaptor::configured(None, None)
                .addressed(Some("AdamSanti"))
                .normalize(
                    &github_headers("ping", None),
                    br#"{"zen":"ok"}"#,
                    SECRET,
                    "ops",
                )
                .expect("normalize"),
        );
        assert!(!event.in_scope);
    }
}

#[path = "webhook/feishu.rs"]
mod feishu;

fn expect_event(outcome: WebhookOutcome) -> NormalizedEvent {
    match outcome {
        WebhookOutcome::Event(event) => event,
        WebhookOutcome::Reply(value) => panic!("expected event, got {value}"),
    }
}

fn sign(secret: &str, body: &[u8]) -> String {
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(body);
    format!("sha256={}", hex::encode(mac.finalize().into_bytes()))
}

fn github_headers(event: &str, signature: Option<&str>) -> HeaderMap {
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

fn github_body() -> &'static [u8] {
    br#"{
        "action":"created",
        "repository":{"full_name":"PerishCode/santi"},
        "issue":{"number":42,"title":"title","assignees":[{"login":"AdamSanti"}]},
        "comment":{"body":"requested work","html_url":"https://github.com/PerishCode/santi/issues/42#issuecomment-1"},
        "sender":{"login":"somehuman"}
    }"#
}

fn feishu_payload(sender_type: &str, open_id: &str, event_type: &str) -> String {
    format!(
        r#"{{
            "schema":"2.0",
            "header":{{"event_id":"ev_123","event_type":"{event_type}","token":"{FEISHU_TOKEN}"}},
            "event":{{
                "sender":{{"sender_type":"{sender_type}","sender_id":{{"open_id":"{open_id}","user_id":"u_1"}}}},
                "message":{{"chat_id":"oc_chat1","chat_type":"p2p","message_id":"om_msg1","content":"top secret"}}
            }}
        }}"#
    )
}

fn encrypt(plaintext: &str) -> String {
    let key = Sha256::digest(FEISHU_KEY.as_bytes());
    let iv = [7u8; 16];
    let ciphertext = cbc::Encryptor::<aes::Aes256>::new_from_slices(&key, &iv)
        .unwrap()
        .encrypt_padded_vec_mut::<Pkcs7>(plaintext.as_bytes());
    let mut data = iv.to_vec();
    data.extend_from_slice(&ciphertext);
    base64::engine::general_purpose::STANDARD.encode(data)
}
