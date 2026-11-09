mod edge;
mod partial;

use std::collections::BTreeMap;

use santi_estate::{
    CallDraft, CompactDraft, Limits, MessageDraft, ReplyDraft, Store, StrandDraft, ThinkingDraft,
    TurnDraft,
};
use santi_model::{compact, message, thinking, turn};

const CREATED: &str = "2026-10-22T00:00:00.000Z";
const LATER: &str = "2026-10-22T00:01:00.000Z";
const LIMITS: Limits = Limits {
    slots: 8,
    settled: 1 << 20,
};

async fn opened(path: &std::path::Path) -> Store {
    let store = super::super::support::bootstrap(path).await;
    store.seed("soul_test", CREATED).await.expect("seed");
    store
        .create_strand(StrandDraft {
            tag: "strand_test",
            soul: "soul_test",
            label: None,
            parent: None,
            fork: None,
            inference: None,
            created: CREATED,
        })
        .await
        .expect("strand");
    store
}

struct Writer<'a>(&'a Store);

impl<'a> Writer<'a> {
    async fn fixed(&self, tag: &str, role: message::Role, actor: &str, text: &str) {
        self.0
            .place(MessageDraft {
                tag,
                strand: "strand_test",
                actor: role,
                actor_id: actor,
                kind: message::Kind::Text,
                content: &message::Content::text(text),
                state: message::State::Fixed,
                request: false,
                created: CREATED,
            })
            .await
            .expect("message");
    }

    async fn turn(&self, tag: &str) {
        self.0
            .create_turn(TurnDraft {
                tag,
                strand: "strand_test",
                trigger: turn::Trigger::System,
                from: 0,
                created: CREATED,
            })
            .await
            .expect("turn");
    }
}

fn round(calls: &[&str], text: usize, continuation: Option<usize>) -> thinking::Round {
    thinking::Round {
        code: Some(200),
        millis: Some(8),
        headers: BTreeMap::new(),
        status: None,
        incomplete: None,
        model: Some("test".into()),
        usage: None,
        text,
        calls: calls.iter().map(|name| name.to_string()).collect(),
        continuation: continuation.map(|count| {
            Box::new(thinking::Continuation {
                calls: count,
                provider: "test".into(),
                model: "test".into(),
                endpoint: "http://test.invalid".into(),
                content: None,
            })
        }),
    }
}

impl<'a> Writer<'a> {
    async fn thought(&self, tag: &str, on: &str, recorded: thinking::Round) {
        self.0
            .create_thinking(ThinkingDraft {
                tag,
                turn: on,
                response: None,
                created: CREATED,
            })
            .await
            .expect("thinking");
        self.0
            .chronicle(tag, &recorded)
            .await
            .expect("chronicle")
            .expect("thinking span");
    }

    async fn call(&self, tag: &str, on: &str) {
        self.0
            .create_call(CallDraft {
                tag,
                turn: on,
                tool: "shell",
                arguments: &serde_json::json!({"command": "true"}),
                created: CREATED,
            })
            .await
            .expect("call");
    }

    async fn result(&self, tag: &str, on: &str) {
        self.0
            .create_reply(ReplyDraft {
                tag,
                call: on,
                output: Some(&serde_json::json!({"stdout": "done"})),
                error: None,
                created: LATER,
            })
            .await
            .expect("result");
    }
}

fn default() -> compact::Exec {
    compact::Exec {
        first: None,
        last: None,
        from: None,
        to: None,
        summary: "settled history".into(),
        absorb: Vec::new(),
        capsule: None,
        dry: false,
    }
}

fn named(first: &str, last: &str) -> compact::Exec {
    compact::Exec {
        first: Some(first.into()),
        last: Some(last.into()),
        ..default()
    }
}

fn absorbing(first: Option<&str>, last: Option<&str>, slots: &[&str]) -> compact::Exec {
    compact::Exec {
        first: first.map(Into::into),
        last: last.map(Into::into),
        absorb: slots.iter().map(|slot| slot.to_string()).collect(),
        ..default()
    }
}

fn seated(from: i64, to: i64) -> compact::Exec {
    compact::Exec {
        from: Some(from),
        to: Some(to),
        ..default()
    }
}

impl<'a> Writer<'a> {
    async fn refuse(&self, request: &compact::Exec, code: &str) -> String {
        let error = self
            .0
            .selection("strand_test", request, LIMITS)
            .await
            .expect_err("selection refuses");
        assert!(error.starts_with(code), "{error}");
        error
    }

    async fn exchange(&self) {
        self.fixed("m0", message::Role::System, "santi", "start")
            .await;
        self.turn("turn_test").await;
        self.thought("s0", "turn_test", round(&["shell"], 6, Some(1)))
            .await;
        self.fixed("m1", message::Role::Soul, "soul_test", "assistant")
            .await;
        self.call("c0", "turn_test").await;
        self.result("r0", "c0").await;
        self.fixed("m2", message::Role::System, "santi", "after")
            .await;
        self.fixed("m3", message::Role::Soul, "soul_test", "latest")
            .await;
    }
}

#[tokio::test]
async fn shrinks() {
    let temp = tempfile::tempdir().expect("temp");
    let store = opened(&temp.path().join("estate.sqlite")).await;
    let writer = Writer(&store);
    writer
        .fixed("m0", message::Role::System, "santi", "start")
        .await;
    writer.turn("turn_test").await;
    writer
        .thought("s0", "turn_test", round(&["shell"], 6, Some(1)))
        .await;
    writer
        .fixed("m1", message::Role::Soul, "soul_test", "assistant")
        .await;
    writer.call("c0", "turn_test").await;
    writer.result("r0", "c0").await;
    writer
        .fixed("m2", message::Role::Soul, "soul_test", "live")
        .await;

    let choice = store
        .selection("strand_test", &default(), LIMITS)
        .await
        .expect("default selection");
    assert_eq!(choice.range.first, "m0");
    assert_eq!(choice.range.last, "m0");
    assert_eq!(choice.requested, None);

    let one = store
        .selection("strand_test", &named("m0", "m0"), LIMITS)
        .await
        .expect("one eligible message remains legal");
    assert_eq!(
        (one.range.first.as_str(), one.range.last.as_str()),
        ("m0", "m0")
    );
}

#[tokio::test]
async fn cuts() {
    let temp = tempfile::tempdir().expect("temp");
    let store = opened(&temp.path().join("estate.sqlite")).await;
    let writer = Writer(&store);
    writer.exchange().await;

    writer
        .refuse(&named("m0", "m1"), "[compact.exchange]")
        .await;
    writer.refuse(&seated(1, 3), "[compact.exchange]").await;
    writer
        .refuse(&named("m1", "m2"), "[compact.exchange]")
        .await;
    writer.refuse(&seated(3, 6), "[compact.exchange]").await;

    let whole = store
        .selection("strand_test", &named("m0", "m2"), LIMITS)
        .await
        .expect("whole exchange");
    assert_eq!(
        (whole.range.first.as_str(), whole.range.last.as_str()),
        ("m0", "m2")
    );

    let preview = store
        .preview_compact("compact_preview", "strand_test", "m0", "m1")
        .await
        .expect_err("preview refuses");
    assert!(preview.contains("[compact.exchange]"), "{preview}");
    let direct = store
        .create_compact(CompactDraft {
            tag: "compact_direct",
            strand: "strand_test",
            first: "m1",
            last: "m2",
            summary: "bypass",
            metadata: None,
            expected: None,
            created: CREATED,
        })
        .await
        .expect_err("create refuses");
    assert!(direct.contains("[compact.exchange]"), "{direct}");
    assert!(
        store
            .compacts("strand_test")
            .await
            .expect("slots")
            .is_empty()
    );
}
