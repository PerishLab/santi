use super::read;

#[test]
fn headers() {
    for kind in [
        "chat_completions",
        "openai_responses",
        "gemini_generate_content",
    ] {
        let source = format!(
            r#"
scenario = "gateway/family/review"
[custody.fixture]
kind = "{kind}"
api_key = "test"
base_url = "http://provider.test"
bytes = 1000
headers = ["x-request-id"]
[custody.fixture.request_headers]
User-Agent = "{{{{meta.user_agent}}}}"
x-session = "{{{{strand.id}}}}"
"#
        );
        let held = read(&source);
        let profile = held.custody["fixture"]
            .resolve("fixture", "wire-model")
            .unwrap();
        let (headers, outgoing) = match profile {
            santi_api::config::Resolved::Completions(p) => (p.headers, p.outgoing),
            santi_api::config::Resolved::Responses(p) => (p.headers, p.outgoing),
            santi_api::config::Resolved::Gemini(p) => (p.headers, p.outgoing),
        };
        assert_eq!(headers, ["x-request-id"]);
        assert_eq!(outgoing["User-Agent"], "{{meta.user_agent}}");
        for value in ["{{unknown}}", "{{strand.id}", r"bad\r\nvalue"] {
            let source = source.replace("{{strand.id}}", value);
            let held = read(&source);
            assert!(
                held.custody["fixture"]
                    .resolve("fixture", "wire-model")
                    .is_err()
            );
        }
    }
}

#[test]
fn retired() {
    use api::config::ConfigPartial;
    use std::io::Write;
    for raw in [
        r#"provider = "gateway""#,
        r#"model = "override""#,
        "[providers.gateway]",
    ] {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(raw.as_bytes()).unwrap();
        assert!(
            radicula_config::load::<ConfigPartial>(file.path()).is_err(),
            "{raw}"
        );
    }
}

#[test]
fn invalid() {
    for value in ["chat", "gateway/Family/review", "gateway/family/a.b.c.d"] {
        let temp = tempfile::tempdir().unwrap();
        let config = temp.path().join("santi.toml");
        std::fs::write(&config, format!("scenario = '{value}'")).unwrap();
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_santi-api"))
            .args(["--config", config.to_str().unwrap(), "bootstrap"])
            .env_clear()
            .env("HOME", temp.path())
            .env("SANTI_HOME", temp.path())
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("scenario"));
        assert!(!temp.path().join("runtime").exists());
    }
}
