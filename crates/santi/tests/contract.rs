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
        "dbd60a4a34e5b92f992c3fe4e1e7e6da7a8bf6161b65d25b66507d4a84ccd248"
    );
}

#[test]
fn template() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("operator/templates/secrets/santi.toml");
    let client: santi::config::Client = plumb::config::load(&path).expect("operator config");
    assert_eq!(
        client.base_url.as_deref(),
        Some("https://santi.liberte.top")
    );
    assert_eq!(client.auth_username.as_deref(), Some("santi-window-cli"));
}
