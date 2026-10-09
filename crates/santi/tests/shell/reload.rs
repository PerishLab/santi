use std::ffi::OsStr;
use std::process::Command;

use santi::config::{Resume, resume};

#[test]
fn identity() {
    let mut command = Command::new("/tmp/santi");
    command
        .arg("tui")
        .env("SANTI_API_KEY", "inherited-key")
        .env("SANTI_AUTH_TOKEN_URL", "https://auth.example/token")
        .env("SANTI_AUTH_CLIENT_ID", "inherited-client")
        .env("SANTI_AUTH_USERNAME", "inherited-user")
        .env("SANTI_AUTH_PASSWORD", "inherited-password");

    resume(
        &mut command,
        Resume {
            base: "http://runtime.example",
            soul: "soul_luna",
            strand: "ss_direct",
            bearer: Some("resolved bearer"),
        },
    );

    assert_eq!(
        command.get_args().collect::<Vec<_>>(),
        [
            "tui",
            "--auth-token-url",
            "",
            "--auth-client-id",
            "",
            "--auth-username",
            "",
            "--auth-password",
            "",
        ]
        .map(OsStr::new),
        "edge credentials are blanked as arguments so no client file can supply them"
    );
    let environment = command.get_envs().collect::<Vec<_>>();
    for (name, expected) in [
        ("SANTI_BASE_URL", "http://runtime.example"),
        ("SANTI_SOUL_ID", "soul_luna"),
        ("SANTI_STRAND_ID", "ss_direct"),
        ("SANTI_API_KEY", "resolved bearer"),
        ("SANTI_AUTH_TOKEN_URL", ""),
        ("SANTI_AUTH_CLIENT_ID", ""),
        ("SANTI_AUTH_USERNAME", ""),
        ("SANTI_AUTH_PASSWORD", ""),
    ] {
        assert!(
            environment.iter().any(|(key, value)| {
                key == &OsStr::new(name) && value == &Some(OsStr::new(expected))
            }),
            "{name} did not have the exact replacement value"
        );
    }
    assert!(
        !command
            .get_args()
            .any(|argument| argument == OsStr::new("resolved bearer")),
        "the resolved bearer must not enter argv"
    );
}

#[test]
fn unresolved() {
    let mut command = Command::new("/tmp/santi");
    command
        .arg("tui")
        .env("SANTI_API_KEY", "inherited-key")
        .env("SANTI_AUTH_TOKEN_URL", "https://auth.example/token")
        .env("SANTI_AUTH_CLIENT_ID", "inherited-client")
        .env("SANTI_AUTH_USERNAME", "inherited-user")
        .env("SANTI_AUTH_PASSWORD", "inherited-password");

    resume(
        &mut command,
        Resume {
            base: "http://runtime.example",
            soul: "soul_luna",
            strand: "ss_direct",
            bearer: None,
        },
    );

    let environment = command.get_envs().collect::<Vec<_>>();
    for name in [
        "SANTI_API_KEY",
        "SANTI_AUTH_TOKEN_URL",
        "SANTI_AUTH_CLIENT_ID",
        "SANTI_AUTH_USERNAME",
        "SANTI_AUTH_PASSWORD",
    ] {
        assert!(
            environment
                .iter()
                .any(|(key, value)| { key == &OsStr::new(name) && value == &Some(OsStr::new("")) }),
            "{name} must be blanked when no bearer was resolved"
        );
    }
    let arguments = command.get_args().collect::<Vec<_>>();
    assert!(
        arguments
            .windows(2)
            .any(|pair| pair == [OsStr::new("--api-key"), OsStr::new("")]),
        "an unresolved bearer is blanked as an argument so no client file can supply one"
    );
}
