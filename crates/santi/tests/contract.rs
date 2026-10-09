use clap::CommandFactory;
use sha2::{Digest, Sha256};

fn render(mut command: clap::Command, path: &str, text: &mut String) {
    let children: Vec<_> = command.get_subcommands().cloned().collect();
    text.push_str(path);
    text.push('\n');
    text.push_str(&command.render_long_help().to_string());
    text.push('\n');
    for child in children {
        let name = child.get_name().to_string();
        render(child, &format!("{path} {name}"), text);
    }
}

#[test]
fn cli() {
    let mut text = String::new();
    render(santi::cli::Cli::command(), "santi", &mut text);
    let hash = format!("{:x}", Sha256::digest(text.as_bytes()));
    assert_eq!(
        hash,
        "1fdaeaea513a1ca46764e76c65b6efda65d1976998dc9514b5d4512d8aeae34d"
    );
}

#[test]
fn template() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("santi.toml");
    std::fs::write(
        &path,
        concat!(
            "base_url = \"https://santi.example.invalid\"\n",
            "auth_token_url = \"https://auth.example.invalid/application/o/token/\"\n",
            "auth_client_id = \"replace-with-client-id\"\n",
            "auth_username = \"santi-window-cli\"\n",
            "auth_password = \"replace-with-app-password\"\n",
        ),
    )
    .expect("write client config");
    let held: santi::config::ClientPartial = plumb::config::load(&path).expect("client config");
    let client = plumb::config::Cascade::merge(santi::config::Client::default(), held);
    assert_eq!(
        client.base_url.as_deref(),
        Some("https://santi.example.invalid")
    );
    assert_eq!(client.auth_username.as_deref(), Some("santi-window-cli"));
    assert!(client.api_key.is_none());
}

#[test]
fn environment() {
    let get = |key: &str| match key {
        "SANTI_BASE_URL" => Some("http://base.example".to_string()),
        "SANTI_API_URL" => Some("http://retired.example".to_string()),
        "SANTI_TOKEN_CACHE" => Some("/tmp/santi-cache.json".to_string()),
        _ => None,
    };
    assert_eq!(santi::config::Client::prefix(), "SANTI");
    let held = <santi::config::Client as plumb::config::Cascade>::lookup("SANTI", &get)
        .expect("client environment");
    let client = plumb::config::Cascade::merge(santi::config::Client::default(), held);
    assert_eq!(client.base_url.as_deref(), Some("http://base.example"));
    assert_eq!(
        client.token_cache.as_deref(),
        Some(std::path::Path::new("/tmp/santi-cache.json"))
    );
}
