use super::*;
use santi_core::render;

#[tokio::test]
async fn rendering() {
    let harness = Harness::open().await;
    let request = || render::Request {
        kind: render::Kind::Skill,
    };
    let skill = harness
        .service
        .render(&harness.strand, request())
        .await
        .unwrap();
    assert_eq!(skill.mime, "text/markdown; charset=utf-8");
    assert!(skill.text.starts_with("---\nname: santi"));
    assert!(skill.text.contains("completed"));
    assert!(skill.text.contains("partial effects"));
    let prompt = harness.prompt().await;
    assert!(prompt.text.contains("santi strand material --kind skill"));
    assert!(prompt.text.contains("santi schema"));
    assert!(!prompt.text.contains(&skill.text));
    harness.soul("An uncertain operating hypothesis, saved by the soul.");
    let next = harness.prompt().await;
    assert!(next.text.contains("An uncertain operating hypothesis"));
    let again = harness
        .service
        .render(&harness.strand, request())
        .await
        .unwrap();
    assert_eq!(again.updated, skill.updated);
    assert_eq!(again.text, skill.text);
    assert!(!again.text.contains("An uncertain operating hypothesis"));
    assert!(
        harness
            .service
            .render("missing", request())
            .await
            .unwrap_err()
            .contains("strand not found")
    );
}
