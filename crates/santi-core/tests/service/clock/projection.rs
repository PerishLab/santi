use super::*;

#[tokio::test]
async fn projects() {
    let temp = tempfile::tempdir().expect("temp dir");
    bootstrap(&temp).await;
    let service = Service::open(config(&temp), Arc::new(Holding::default()))
        .await
        .expect("open service")
        .bounded(santi_core::budget::Execution {
            profile: "test".to_string(),
            rounds: 16,
            calls: 256,
            output: 4096,
            shell: 1024,
            feedback_after_calls: None,
            feedback_command: None,
            feedback_cwd: None,
        })
        .expect("bound service");
    let soul = service
        .awaken(soul::Draft { memory: None })
        .await
        .expect("awaken soul");
    let mut held = Vec::new();
    for index in 0..18 {
        let strand = service.seat(&soul.id).await.expect("seat soul").strand;
        let posted = service
            .send(
                &strand.id,
                strand::Post {
                    content: vec![message::Part::Text {
                        text: format!("hold {index}"),
                    }],
                },
            )
            .await
            .expect("send held turn");
        held.push((strand, accepted_turn(&posted).id.clone()));
    }
    let foreign = service
        .awaken(soul::Draft { memory: None })
        .await
        .expect("awaken foreign soul");
    let strand = service
        .seat(&foreign.id)
        .await
        .expect("foreign seat")
        .strand;
    let posted = service
        .send(
            &strand.id,
            strand::Post {
                content: vec![message::Part::Text {
                    text: "hold foreign".to_string(),
                }],
            },
        )
        .await
        .expect("send foreign turn");
    let foreign = (strand.id, accepted_turn(&posted).id.clone());
    let excluded = held[0].0.id.clone();
    let mut projection = None;
    for _ in 0..200 {
        let current = service
            .running(&soul.id, Some(&excluded))
            .await
            .expect("project turns")
            .expect("known soul");
        if current.total == 17 {
            projection = Some(current);
            break;
        }
        sleep(Duration::from_millis(10)).await;
    }
    let projection = projection.expect("turns became active");
    assert_eq!(projection.total, 17);
    assert!(projection.truncated);
    assert_eq!(projection.active.len(), 16);
    for entry in &projection.active {
        assert_ne!(entry.strand.id, excluded);
        assert_ne!(entry.strand.id, foreign.0);
        assert_eq!(entry.turn.status, santi_core::turn::Status::Running);
        assert_eq!(entry.execution.as_ref().expect("budget").rounds, 16);
        assert!(entry.usage.is_some());
    }
    assert!(projection.active.windows(2).all(|pair| {
        (&pair[0].turn.created, &pair[0].turn.id) <= (&pair[1].turn.created, &pair[1].turn.id)
    }));
    assert!(
        service
            .running("soul_missing", None)
            .await
            .expect("missing soul")
            .is_none()
    );
    for (_, turn) in held {
        service.stop(&turn).await.expect("stop turn");
    }
    service.stop(&foreign.1).await.expect("stop foreign turn");
    service.quiesce(Duration::ZERO);
    service.drain().await;
}
