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
        "b4f4014327ddcc972f257a953343c2a0c062d591cb883c6bf0f3797abe63fc09"
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
    let client: santi::config::Client = plumb::config::load(&path).expect("client config");
    assert_eq!(
        client.base_url.as_deref(),
        Some("https://santi.example.invalid")
    );
    assert_eq!(client.auth_username.as_deref(), Some("santi-window-cli"));
    assert!(client.api_key.is_none());
}
