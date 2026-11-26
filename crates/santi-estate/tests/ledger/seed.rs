use santi_error::{Draft, Scope, Source, catalog};
use santi_estate::{
    Arrival, Call, Compact, Downstream, Effect, Environ, Finish, Message, Outbox, Redemption,
    Replay, Store, Strand, Thinking, Turn, Webhook,
};
use santi_model::alarm::{self, Occurrence};
use santi_model::{effect, environ, event, message, turn};

pub(super) const SUDO: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
pub(super) const FIRST: &str = "2026-07-28T00:00:00.000Z";
pub(super) const LATER: &str = "2026-07-28T00:01:00.000Z";
pub(super) const SOUL: &str = "soul_test";

pub(super) async fn seed(store: &Store) {
    store.seed(SOUL, FIRST).await.expect("seed");
    store
        .subscribe(Webhook {
            name: "github",
            adaptor: "github",
            soul: SOUL,
            strategy: "per_thread",
            credential: "GITHUB_SECRET",
            created: FIRST,
        })
        .await
        .expect("subscribe");
    store
        .enroll(Downstream {
            tag: "worker",
            prefix: "worker/",
            digest: "credential_digest",
            created: FIRST,
        })
        .await
        .expect("enroll");
}

pub(super) async fn populate(store: &Store, strand: &str) {
    store
        .weave(Strand {
            tag: strand,
            soul: SOUL,
            label: Some(strand),
            parent: None,
            fork: None,
            inference: None,
            created: FIRST,
        })
        .await
        .expect("strand");
    let id = |name: &str| format!("{name}_{strand}");
    let hello = message::Content::text("hello");
    let received = store
        .accept(inbox(&id("inbox"), strand, &hello), 10)
        .await
        .expect("inbox");
    let turn = store
        .start(Turn {
            tag: &id("turn"),
            strand,
            trigger: turn::Trigger::Send,
            from: 0,
            created: FIRST,
        })
        .await
        .expect("turn");
    store
        .advance(santi_estate::Receipt {
            inbox: &received.id,
            state: santi_model::receipt::State::Driving,
            turn: Some(&turn.id),
            incident: None,
            occurred: FIRST,
        })
        .await
        .expect("receipt");
    store
        .place(Message {
            tag: &id("message"),
            strand,
            actor: message::Role::System,
            sender: "santi",
            kind: message::Kind::Text,
            content: &message::Content::text("hello"),
            state: message::State::Fixed,
            request: false,
            created: FIRST,
        })
        .await
        .expect("message");
    store
        .condense(Compact {
            tag: &id("compact"),
            strand,
            first: &id("message"),
            last: &id("message"),
            summary: "summary",
            metadata: None,
            expected: None,
            created: FIRST,
        })
        .await
        .expect("compact");
    store
        .ponder(Thinking {
            tag: &id("thinking"),
            turn: &turn.id,
            response: None,
            created: FIRST,
        })
        .await
        .expect("thinking");
    let call = store
        .calling(Call {
            tag: &id("call"),
            turn: &turn.id,
            tool: "shell",
            arguments: &serde_json::json!({"command": "true"}),
            created: FIRST,
        })
        .await
        .expect("call");
    let applied = store
        .stage(Effect {
            tag: &id("effect"),
            turn: &turn.id,
            call: Some(&call.id),
            kind: "shell",
            metadata: None,
            created: FIRST,
        })
        .await
        .expect("effect");
    store
        .dispatching(&applied.id, LATER)
        .await
        .expect("dispatch");
    store
        .redeem(
            &applied.id,
            Redemption {
                result: &id("result"),
                call: &call.id,
                output: Some(&serde_json::json!({"stdout": "done"})),
                error: None,
                outcome: effect::Outcome::Applied,
                occurred: LATER,
            },
        )
        .await
        .expect("redeem");
    super::job::job(store, strand, &turn.id).await;
    let alarm = store
        .arm(&alarm::Draft {
            soul: SOUL.into(),
            strand: strand.into(),
            name: "follow".into(),
            text: "look".into(),
            cron: "*/15 * * * *".into(),
            generation: None,
        })
        .await
        .expect("alarm");
    store
        .fire(
            &alarm.id,
            &Occurrence {
                generation: 1,
                minute: 29_000_000,
            },
        )
        .await
        .expect("occurrence");
    store
        .replay(
            inbox(&id("inbox_webhook"), strand, &hello),
            Replay::Webhook {
                subscription: "github",
                delivery: &id("delivery"),
                digest: &id("request"),
            },
            10,
        )
        .await
        .expect("delivery");
    store
        .replay(
            inbox(&id("inbox_downstream"), strand, &hello),
            Replay::Downstream {
                owner: "worker",
                request: &id("worker/request"),
                digest: &id("downstream"),
            },
            10,
        )
        .await
        .expect("ingest");
    store
        .export(Environ {
            scope: environ::Scope::Strand,
            owner: strand,
            name: "STRAND_VALUE",
            value: strand,
            occurred: FIRST,
        })
        .await
        .expect("strand environ");
    store
        .raise(failure(&Scope::new("strand", strand)), FIRST)
        .await
        .expect("strand incident");
    store
        .record(santi_estate::object::Draft {
            strand,
            key: "avatars/santi.svg",
            length: 4,
            sha256: "digest",
            occurred: FIRST,
        })
        .await
        .expect("material");
    store
        .finish(Finish {
            turn: &turn.id,
            reply: None,
            provider: "test",
            model: "test",
            response: None,
            occurred: LATER,
        })
        .await
        .expect("finish");
    store
        .enqueue(Outbox {
            stream: "events",
            event: &event::Event {
                id: id("event"),
                strand: strand.to_string(),
                turn: turn.id.clone(),
                label: strand.to_string(),
                text: "reply".to_string(),
                completed: LATER.to_string(),
            },
        })
        .await
        .expect("outbox");
}

pub(super) fn inbox<'a>(
    tag: &'a str,
    strand: &'a str,
    content: &'a message::Content,
) -> Arrival<'a> {
    Arrival {
        tag,
        strand,
        kind: message::Kind::Text,
        content,
        source: None,
        created: FIRST,
    }
}

pub(super) fn failure(scope: &Scope) -> Draft {
    Draft {
        key: catalog::INTERNAL.key(&scope.kind, &scope.id),
        descriptor: catalog::INTERNAL,
        scope: scope.clone(),
        source: Source::new("santi-estate", "test"),
        message: "failed".to_string(),
        context: serde_json::json!({}),
    }
}
