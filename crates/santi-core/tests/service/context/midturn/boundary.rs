use super::super::layers::{Selection, declare};
use super::super::*;
use super::world::*;
#[tokio::test]
async fn boundary() {
    let f = open(Ending::Tools).await;
    let Fixture {
        service,
        rounds,
        store,
        strand,
        ..
    } = &f;
    let start = send(service, strand, "start").await;
    entered(rounds, 0).await;
    let receipt = service
        .receipt(&start.receipt.inbox)
        .await
        .unwrap()
        .unwrap();
    let turn = receipt
        .transitions
        .iter()
        .find_map(|t| t.turn.clone())
        .unwrap();
    let before = store.inference(&turn).await.unwrap().unwrap();
    declare(
        store,
        strand,
        Selection {
            provider: "beta",
            model: "beta-model",
            effort: "high",
        },
    )
    .await;
    let extra = send(service, strand, "unique_inbox_marker_369").await;
    assert_eq!(
        service
            .receipt(&extra.receipt.inbox)
            .await
            .unwrap()
            .unwrap()
            .state,
        santi_core::receipt::State::Accepted
    );
    assert_eq!(markers(&rounds.requests.lock().unwrap()[0].2), 0);
    rounds.release[0].notify_one();
    entered(rounds, 1).await;
    {
        let requests = rounds.requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert_eq!(markers(&requests[1].2), 1);
        assert_eq!(requests[1].0, "alpha");
        assert_eq!(requests[1].1.as_deref(), Some("low"));
        assert_eq!(requests[1].2.model, "alpha-model");
        let calls: Vec<_> = requests[1]
            .2
            .input
            .iter()
            .enumerate()
            .filter_map(|(i, item)| match item {
                Item::Call { call, .. } if call == "boundary_query" => Some(i),
                _ => None,
            })
            .collect();
        let outputs: Vec<_> = requests[1]
            .2
            .input
            .iter()
            .enumerate()
            .filter_map(|(i, item)| match item {
                Item::Output { call, .. } if call == "boundary_query" => Some(i),
                _ => None,
            })
            .collect();
        let allcalls = requests[1]
            .2
            .input
            .iter()
            .filter(|i| matches!(i, Item::Call { .. }))
            .count();
        let alloutputs = requests[1]
            .2
            .input
            .iter()
            .filter(|i| matches!(i, Item::Output { .. }))
            .count();
        assert_eq!((allcalls, alloutputs), (2, 2));
        let marker = requests[1].2.input.iter().position(|i| matches!(i, Item::Message { content, .. } if content.contains("unique_inbox_marker_369"))).unwrap();
        for (index, item) in requests[1].2.input.iter().enumerate() {
            if matches!(item, Item::Call { .. } | Item::Output { .. }) {
                assert!(index < marker);
            }
        }
        assert_eq!(calls.len(), 1);
        assert_eq!(outputs.len(), 1);
        assert!(calls[0] < outputs[0]);
    }
    let supplement = service
        .receipt(&extra.receipt.inbox)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(supplement.state, santi_core::receipt::State::Driving);
    let bindings: Vec<_> = supplement
        .transitions
        .iter()
        .filter(|t| t.state == santi_core::receipt::State::Driving)
        .collect();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].turn.as_deref(), Some(turn.as_str()));
    let events = store.events(strand).await.unwrap();
    let drains: Vec<_> = events
        .iter()
        .filter(|e| e.payload["kind"] == "inbox_drain" && e.payload["inbox"] == extra.receipt.inbox)
        .collect();
    assert_eq!(drains.len(), 1);
    let message = &drains[0].message;
    assert_eq!(
        events
            .iter()
            .filter(|e| &e.message == message && e.action == "prepared")
            .filter(|e| e.payload["kind"] == "input_prepared")
            .filter(|e| e.payload["round"] == 2 && e.payload["turn"] == turn)
            .count(),
        1
    );
    let after = store.inference(&turn).await.unwrap().unwrap();
    assert_eq!(before, after);
    rounds.release[1].notify_one();
    super::super::layers::until(store, strand, santi_core::turn::Status::Completed).await;
    let next = send(service, strand, "next turn").await;
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if service
                .receipt(&next.receipt.inbox)
                .await
                .unwrap()
                .unwrap()
                .state
                == santi_core::receipt::State::Completed
            {
                break;
            }
            sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    let requests = rounds.requests.lock().unwrap();
    assert_eq!(requests[2].0, "beta");
    assert_eq!(requests[2].1.as_deref(), Some("high"));
    eprintln!(
        "candidate boundary: round2 marker=1; supplement Driving once on original Turn; input_prepared round2; complete call/output; frozen alpha/low; next Turn beta/high"
    );
}
