use super::*;

const SOURCE: &str = r#"
provider = "old"
[providers.old]
kind = "chat_completions"
base_url = "https://gateway.invalid"
model = "approved.model"
api_key = "hidden-key"
request_headers = {"X-Private" = "hidden-header"}
[custody.operator]
kind = "chat_completions"
base_url = "https://gateway.invalid"
model = "hidden-default"
bytes = 2345678
headers = ["hidden-selection"]
request_headers = {"X-Other" = "hidden-template"}
api_key = "hidden-custody"
"#;
const MAP: &str = r#"
[profiles.old]
provider = "gateway"
access = "route"
custody = "operator"
aliases = ["old"]
offerings = {"approved.model" = "operator-choice"}
"#;

#[test]
fn custody() {
    let private = SOURCE.replace("approved.model", "hidden-profile-default");
    let mapping = format!("{MAP}\ndefault_in_custody = true");
    let (graph, coverage) = legacy::convert(&private, &mapping).unwrap();
    let mut estate = Estate::default();
    let plan = estate.plan(graph).unwrap();
    assert_eq!(plan.len(), 5);
    let mut wire = Vec::new();
    for row in &plan {
        let native = estate.native(row).unwrap();
        let mut reply = native.clone();
        reply["id"] = json!(1);
        estate.created(row, &native, reply).unwrap();
        wire.push(native);
    }
    let public = json!([wire, coverage, plan]).to_string();
    for forbidden in [
        "hidden-",
        "X-Private",
        "X-Other",
        "2345678",
        "request_headers",
        "api_key",
    ] {
        assert!(!public.contains(forbidden));
    }
    assert!(public.contains("custody://operator"));
}

#[test]
fn coverage() {
    for (source, mapping, expected) in [
        (
            SOURCE.into(),
            MAP.replace("profiles.old", "profiles.other"),
            "missing",
        ),
        (
            SOURCE.into(),
            MAP.replace("approved.model", "different"),
            "model not covered",
        ),
        (
            format!("{SOURCE}\n[souls.s.inference]\nmodel='hidden-unmapped'"),
            MAP.into(),
            "model not covered",
        ),
        (SOURCE.into(), MAP.replace("[\"old\"]", "[]"), "preserve"),
        (
            SOURCE.replace(
                "https://gateway.invalid",
                "https://gateway.invalid/?key=hidden-key",
            ),
            MAP.into(),
            "invalid access endpoint",
        ),
        (
            SOURCE.replacen("https://gateway.invalid", "https://different.invalid", 1),
            MAP.into(),
            "binding mismatch",
        ),
        (
            format!("{SOURCE}\n invalid hidden-key"),
            MAP.into(),
            "invalid legacy",
        ),
    ] {
        let error = legacy::convert(&source, &mapping)
            .err()
            .unwrap()
            .to_string();
        assert!(error.contains(expected), "{error}");
        assert!(!error.contains("hidden-"));
    }
}

fn catalog(id: i64) -> Vec<Value> {
    vec![
        json!({"kind":"preference", "candidate":id,"id":id}),
        json!({"kind":"candidate", "offering":id,"scenario":id,"evidence":"tested", "id":id}),
        json!({"kind":"offering", "access":id,"family":id,"model":"vendor/m:1","id":id}),
        json!({"kind":"alias", "access":id,"name":"alias","id":id}),
        json!({"kind":"access", "name":"route","provider":id,"protocol":"openai_responses", "endpoint":"https://gateway.invalid", "credential":"custody://local", "id":id}),
        json!({"kind":"family", "name":"family","id":id}),
        json!({"kind":"scenario", "name":"recall","criterion":"accuracy","id":id}),
        json!({"kind":"provider", "name":"provider","id":id}),
    ]
}

#[test]
fn resume() {
    let old = Estate::snapshot(catalog(111)).unwrap();
    let archive = json!({"version":1,"resources":old.graph.rows().collect::<Vec<_>>()}).to_string();
    assert!(!archive.contains("111"));
    let mut local = Vec::new();
    loop {
        let mut target = Estate::snapshot(local.clone()).unwrap();
        let plan = target.plan(portable(&archive).unwrap()).unwrap();
        assert_eq!(plan.len(), 8 - local.len());
        let Some(row) = plan.first() else { break };
        let native = target.native(row).unwrap();
        let mut reply = native.clone();
        reply["id"] = json!(local.len() + 500);
        target.created(row, &native, reply.clone()).unwrap();
        local.push(reply);
    }
    let new = Estate::snapshot(local.clone()).unwrap();
    assert!(new.graph.rows().eq(old.graph.rows()));
    local.iter_mut().find(|r| r["kind"] == "candidate").unwrap()["evidence"] = json!("manual");
    assert!(
        Estate::snapshot(local)
            .unwrap()
            .plan(portable(&archive).unwrap())
            .is_err()
    );
}

#[test]
fn malformed() {
    for (index, field, value) in [
        (1, "offering", json!(999)),
        (4, "request_headers", json!({})),
        (4, "credential", json!("raw-value")),
    ] {
        let mut rows = catalog(12);
        rows[index][field] = value;
        assert!(Estate::snapshot(rows).is_err());
    }
}

#[test]
fn conflicts() {
    let mut local = catalog(17);
    local.iter_mut().find(|r| r["kind"] == "offering").unwrap()["model"] = json!("operator-choice");
    let mut estate = Estate::snapshot(local).unwrap();
    assert!(
        estate
            .plan(Estate::snapshot(catalog(8)).unwrap().graph)
            .unwrap_err()
            .to_string()
            .contains("conflicting scenario preferences")
    );
}
