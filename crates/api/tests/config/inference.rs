use super::{Providers, read};

#[test]
fn headers() {
    for kind in [
        "chat_completions",
        "openai_responses",
        "gemini_generate_content",
    ] {
        let source = format!(
            r#"
provider = "fixture"
[providers.fixture]
kind = "{kind}"
api_key = "test"
model = "test"
base_url = "http://provider.test"
bytes = 1000
headers = ["x-request-id"]
[providers.fixture.request_headers]
User-Agent = "{{{{meta.user_agent}}}}"
x-session = "{{{{strand.id}}}}"
"#
        );
        let held = read(&source);
        let profile = held.providers["fixture"].resolve("fixture").unwrap();
        let (headers, outgoing) = match profile {
            santi_api::config::Resolved::ChatCompletions(p) => (p.headers, p.outgoing),
            santi_api::config::Resolved::OpenAiResponses(p) => (p.headers, p.outgoing),
            santi_api::config::Resolved::Gemini(p) => (p.headers, p.outgoing),
        };
        assert_eq!(headers, ["x-request-id"]);
        assert_eq!(outgoing["User-Agent"], "{{meta.user_agent}}");
        for value in ["{{unknown}}", "{{strand.id}", r"bad\r\nvalue"] {
            let source = source.replace("{{strand.id}}", value);
            let held = read(&source);
            assert!(held.providers["fixture"].resolve("fixture").is_err());
        }
    }
}

#[test]
fn rebases() {
    use santi_core::inference::{Declaration, Effective, Origin};

    let held = read(
        r#"
provider = "deepseek"
model = "service-model"
reasoning_effort = "high"

[providers.deepseek]
kind = "chat_completions"
api_key = "test"
model = "provider-model"
base_url = "http://127.0.0.1:1"
reasoning_effort = "medium"
bytes = 120000
"#,
    );
    assert_eq!(held.model.as_deref(), Some("service-model"));
    assert_eq!(held.reasoning_effort.as_deref(), Some("high"));
    let base = held.providers["deepseek"].resolve("deepseek").unwrap();
    assert_eq!(base.model(), "provider-model");
    assert_eq!(base.effort(), Some("medium"));

    let service = Declaration {
        provider: Some(held.provider.clone()),
        model: held.model.clone(),
        reasoning_effort: held.reasoning_effort.clone(),
    };
    let effective =
        Effective::resolve(&[(&service, Origin::Service)], &Providers(&held.providers)).unwrap();
    assert_eq!(effective.provider, "deepseek");
    assert_eq!(effective.model, "service-model");
    assert_eq!(effective.reasoning_effort.as_deref(), Some("high"));
    assert_eq!(effective.source.provider, Origin::Service);
    assert_eq!(effective.source.model, Origin::Service);
    assert_eq!(effective.source.reasoning_effort, Some(Origin::Service));

    let rebased = base.rebase(&effective).unwrap();
    assert_eq!(rebased.model(), "service-model");
    assert_eq!(rebased.effort(), Some("high"));
    assert_eq!(rebased.bytes(), base.bytes());
}

#[test]
fn defaults() {
    use santi_core::inference::{Declaration, Effective, Origin};

    let held = read(
        r#"
provider = "deepseek"

[providers.deepseek]
kind = "chat_completions"
api_key = "test"
model = "provider-model"
base_url = "http://127.0.0.1:1"
reasoning_effort = "medium"
bytes = 120000
"#,
    );
    assert!(held.model.is_none());
    assert!(held.reasoning_effort.is_none());
    let service = Declaration {
        provider: Some(held.provider.clone()),
        model: None,
        reasoning_effort: None,
    };
    let effective =
        Effective::resolve(&[(&service, Origin::Service)], &Providers(&held.providers)).unwrap();
    assert_eq!(effective.model, "provider-model");
    assert_eq!(effective.reasoning_effort.as_deref(), Some("medium"));
    assert_eq!(effective.source.model, Origin::Default);
    assert_eq!(effective.source.reasoning_effort, Some(Origin::Default));
}

#[test]
fn switch() {
    use santi_api::config::Resolved;
    use santi_core::inference::{Declaration, Effective, Origin};

    let held = read(
        r#"
provider = "deepseek"

[providers.deepseek]
kind = "chat_completions"
api_key = "test"
model = "deepseek-chat"
base_url = "http://127.0.0.1:1"
reasoning_effort = "medium"
bytes = 120000

[providers.openai]
kind = "openai_responses"
api_key = "test"
model = "gpt-5"
base_url = "http://127.0.0.1:1"
reasoning_effort = ""
summary = ""
bytes = 120000
"#,
    );
    let deepseek = Declaration {
        provider: Some("deepseek".into()),
        model: None,
        reasoning_effort: None,
    };
    let effective =
        Effective::resolve(&[(&deepseek, Origin::Service)], &Providers(&held.providers)).unwrap();
    assert_eq!(effective.provider, "deepseek");
    assert_eq!(effective.model, "deepseek-chat");
    assert_eq!(effective.reasoning_effort.as_deref(), Some("medium"));
    assert_eq!(effective.source.model, Origin::Default);
    assert_eq!(effective.source.reasoning_effort, Some(Origin::Default));

    let base = held.providers["deepseek"].resolve("deepseek").unwrap();
    let rebased = base.rebase(&effective).unwrap();
    assert!(matches!(rebased, Resolved::ChatCompletions(_)));
    assert_eq!(rebased.model(), "deepseek-chat");
    assert_eq!(rebased.effort(), Some("medium"));
    assert_eq!(rebased.bytes(), 120000);

    let openai = Declaration {
        provider: Some("openai".into()),
        model: None,
        reasoning_effort: None,
    };
    let effective =
        Effective::resolve(&[(&openai, Origin::Service)], &Providers(&held.providers)).unwrap();
    assert_eq!(effective.provider, "openai");
    assert_eq!(effective.model, "gpt-5");
    assert_eq!(effective.reasoning_effort, None);
    assert_eq!(effective.source.reasoning_effort, None);

    let base = held.providers["openai"].resolve("openai").unwrap();
    assert_eq!(base.model(), "gpt-5");
    assert_eq!(base.effort(), None);
    let rebased = base.rebase(&effective).unwrap();
    assert!(matches!(rebased, Resolved::OpenAiResponses(_)));
    assert_eq!(rebased.model(), "gpt-5");
    assert_eq!(rebased.effort(), None);

    let explicit = Declaration {
        provider: Some("openai".into()),
        model: Some("service-model".into()),
        reasoning_effort: Some("high".into()),
    };
    let effective =
        Effective::resolve(&[(&explicit, Origin::Service)], &Providers(&held.providers)).unwrap();
    assert_eq!(effective.model, "service-model");
    assert_eq!(effective.source.model, Origin::Service);
    assert_eq!(effective.reasoning_effort.as_deref(), Some("high"));
    assert_eq!(effective.source.reasoning_effort, Some(Origin::Service));
}

#[test]
fn custody() {
    let held = read(
        r#"
provider = "gateway/family/review"
[custody.operator]
kind = "chat_completions"
base_url = "https://fixture.invalid/v1"
model = "legacy-default"
api_key = "fixture"
bytes = 12000
replay = "history"
"#,
    );
    assert!(held.providers.is_empty());
    assert_eq!(held.custody["operator"].kind(), "chat_completions");
}
