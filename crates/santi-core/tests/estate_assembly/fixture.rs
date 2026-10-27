use super::*;
use serde_json::{Value, json};

pub struct Frame {
    pub temp: tempfile::TempDir,
    pub store: Store,
    pub expected: Vec<Value>,
    pub middle: usize,
}

impl Frame {
    pub async fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::bootstrap(
            temp.path().join("estate.sqlite"),
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )
        .await
        .unwrap();
        store.seed("soul_test", FIRST).await.unwrap();
        store
            .create_strand(StrandDraft {
                tag: "strand_test",
                soul: "soul_test",
                label: None,
                parent: None,
                fork: None,
                created: FIRST,
            })
            .await
            .unwrap();
        place(
            &store,
            "message_first",
            message::Role::System,
            "begin 工具🙂",
        )
        .await;
        store
            .create_turn(TurnDraft {
                tag: "turn_test",
                strand: "strand_test",
                trigger: turn::Trigger::System,
                from: 1,
                created: FIRST,
            })
            .await
            .unwrap();
        let mut expected = vec![
            json!({"kind":"message","role":"user","content":"[message message_first]\nbegin 工具🙂"}),
        ];
        let mut middle = 0;
        for index in 0usize..140 {
            let tag = format!("thinking_{index}");
            store
                .create_thinking(ThinkingDraft {
                    tag: &tag,
                    turn: "turn_test",
                    response: None,
                    created: FIRST,
                })
                .await
                .unwrap();
            let retained = thinking::Continuation {
                calls: 1,
                provider: "synthetic".into(),
                model: "fixed".into(),
                endpoint: "http://provider.invalid".into(),
                content: Some(format!("retained 工具🙂 {index}")),
            };
            let continuation = index.is_multiple_of(2).then(|| Box::new(retained.clone()));
            store
                .chronicle(
                    &tag,
                    &thinking::Round {
                        continuation,
                        calls: vec!["shell".into()],
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
            store
                .complete_thinking(&tag, thinking::Reason::Called, LATER)
                .await
                .unwrap();
            expected.push(if index.is_multiple_of(2) {
                json!({"kind":"continuation","value":retained})
            } else {
                json!({"kind":"boundary"})
            });
            pair(&store, &mut expected, index).await;
            if index == 69 {
                middle = expected.len();
                place(&store, "message_middle", message::Role::System, "middle").await;
                expected.push(json!({"kind":"message","role":"user","content":"[message message_middle]\nmiddle"}));
            }
        }
        place(
            &store,
            "message_last",
            message::Role::Soul,
            "[message historical]\nfinal 工具🙂",
        )
        .await;
        expected.push(json!({"kind":"message","role":"assistant","content":"final 工具🙂"}));
        let end = store.strand("strand_test").await.unwrap().unwrap().next - 1;
        store.complete_turn("turn_test", end, LATER).await.unwrap();
        Self {
            temp,
            store,
            expected,
            middle,
        }
    }
}

async fn place(store: &Store, tag: &str, actor: message::Role, text: &str) {
    store
        .place(MessageDraft {
            tag,
            strand: "strand_test",
            actor,
            actor_id: "synthetic",
            kind: message::Kind::Text,
            content: &message::Content::text(text),
            state: message::State::Fixed,
            request: false,
            created: FIRST,
        })
        .await
        .unwrap();
}

async fn pair(store: &Store, expected: &mut Vec<Value>, index: usize) {
    let call = format!("call_{:04}", 1000 - index);
    let arguments = json!({"index":index});
    store
        .create_call(CallDraft {
            tag: &call,
            turn: "turn_test",
            tool: "shell",
            arguments: &arguments,
            created: FIRST,
        })
        .await
        .unwrap();
    expected.push(json!({"kind":"call","call":call,"name":"shell","raw":arguments.to_string()}));
    let output = index
        .is_multiple_of(2)
        .then_some(json!({"sequence":index,"text":"工具🙂"}));
    let error = (!index.is_multiple_of(2)).then_some("synthetic failure");
    store
        .create_reply(ReplyDraft {
            tag: &format!("reply_{index}"),
            call: &call,
            output: output.as_ref(),
            error,
            created: LATER,
        })
        .await
        .unwrap();
    let reported = json!({"ok":error.is_none(),"output":output,"error":error,"timing":{"called_at":FIRST,"replied_at":LATER,"call_to_result_ms":60000}}).to_string();
    expected.push(json!({"kind":"output","call":call,"output":reported}));
}
