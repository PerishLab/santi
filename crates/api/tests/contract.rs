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

fn canonical(text: &str) -> String {
    text.lines()
        .map(|line| {
            let Some(start) = line.find("[env: ") else {
                return line.to_string();
            };
            let Some(end) = line[start..].find(']').map(|offset| start + offset) else {
                return line.to_string();
            };
            let owner = &line[start + "[env: ".len()..end];
            let name = owner.split('=').next().unwrap_or(owner);
            format!("{}[env: {name}]{}", &line[..start], &line[end + 1..])
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn cli() {
    let mut text = String::new();
    render(api::cli::Cli::command(), "santi-api", &mut text);
    let hash = format!("{:x}", Sha256::digest(canonical(&text).as_bytes()));
    assert_eq!(
        hash,
        "f5ffcfa3104221dc4a38f7d1d03388f8f59fe66404ac16a54f0b3b9359ebbfe2"
    );
}
