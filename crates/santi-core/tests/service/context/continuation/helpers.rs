use super::*;

fn exchanged(id: &str, arguments: serde_json::Value) -> Vec<Event> {
    vec![
        Event::Started {
            response: Some(id.into()),
        },
        Event::Thinking("display summary".into()),
        Event::Text("compacting".into()),
        Event::Retained(state("original compact", 1)),
        Event::Called(Call {
            response: id.into(),
            mark: None,
            item: json!({"type": "function_call"}),
            call: id.into(),
            name: "compact".into(),
            raw: arguments.to_string(),
            arguments,
        }),
        Event::Completed {
            response: Some(id.into()),
        },
    ]
}

pub(super) fn explicit(call: &str, summary: &str, first: &str, last: &str) -> Vec<Event> {
    exchanged(
        call,
        json!({"summary": summary, "first": first, "last": last}),
    )
}

pub(super) fn inflated(mut events: Vec<Event>, bytes: usize) -> Vec<Event> {
    for event in &mut events {
        if let Event::Retained(state) = event {
            state.content = Some("s".repeat(bytes));
        }
    }
    events
}

pub(super) fn offers(request: &Request) -> bool {
    request.tools.as_deref().is_some_and(|tools| {
        tools.iter().any(
            |tool| matches!(tool, santi_provider::Tool::Function(function) if function.name == "compact"),
        )
    })
}

pub(super) async fn maintained(service: &Service, strand: &str) -> String {
    for _ in 0..500 {
        let snapshot = service.snapshot(strand).await.unwrap().unwrap();
        if let Some(call) = snapshot.calls.iter().find(|call| call.tool == "compact") {
            return call.turn.clone();
        }
        if snapshot
            .turns
            .iter()
            .any(|turn| turn.status == santi_core::turn::Status::Failed)
        {
            let codes = snapshot
                .errors
                .iter()
                .map(|incident| incident.code.clone())
                .collect::<Vec<_>>();
            panic!("turn failed before a compact call: {codes:?}");
        }
        sleep(Duration::from_millis(20)).await;
    }
    panic!("maintenance did not record a compact call");
}
