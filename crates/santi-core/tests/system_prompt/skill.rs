use super::*;

#[tokio::test]
async fn materials() {
    let harness = PromptHarness::open().await;
    let request = || material::Request {
        kind: material::Kind::Skill,
    };
    let skill = harness
        .service
        .material(&harness.strand, request())
        .await
        .unwrap();
    assert_eq!(skill.content_type, "text/markdown; charset=utf-8");
    assert!(skill.text.starts_with("---\nname: santi"));
    assert!(skill.text.contains("completed"));
    assert!(skill.text.contains("partial effects"));
    let prompt = harness.system_prompt().await;
    assert!(prompt.text.contains("santi strand material --kind skill"));
    assert!(prompt.text.contains("santi schema"));
    assert!(!prompt.text.contains(&skill.text));
    harness.write_soul("An uncertain operating hypothesis, saved by the soul.");
    let next = harness.system_prompt().await;
    assert!(next.text.contains("An uncertain operating hypothesis"));
    let again = harness
        .service
        .material(&harness.strand, request())
        .await
        .unwrap();
    assert_eq!(again.updated, skill.updated);
    assert_eq!(again.text, skill.text);
    assert!(!again.text.contains("An uncertain operating hypothesis"));
    assert!(
        harness
            .service
            .material("missing", request())
            .await
            .unwrap_err()
            .contains("strand not found")
    );
}
