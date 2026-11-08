pub(crate) fn inspect(base: &str, strand: &str, receipt: Option<&str>) -> String {
    let Some(runtime) = command(base, "strand runtime", strand) else {
        return "select a credential-free HTTP(S) service URL explicitly with --base-url to inspect this receipt and Strand; no command was generated".into();
    };
    match receipt.and_then(|id| command(base, "receipt", id)) {
        Some(receipt) => format!("inspect with `{receipt}` and `{runtime}`"),
        None => format!("inspect with `{runtime}`"),
    }
}

pub(crate) fn drive(base: &str, strand: &str) -> String {
    match command(base, "strand drive", strand) {
        Some(command) => format!("explicitly redrive with `{command}`"),
        None => {
            "select the intended credential-free service URL before explicitly redriving".into()
        }
    }
}

fn command(base: &str, operation: &str, id: &str) -> Option<String> {
    let url = reqwest::Url::parse(base).ok()?;
    match (url.scheme(), url.host_str()) {
        ("http" | "https", Some(_)) => {}
        _ => return None,
    }
    match (url.username(), url.password()) {
        ("", None) => {}
        _ => return None,
    }
    match (url.query(), url.fragment()) {
        (None, None) => {}
        _ => return None,
    }
    if base.chars().chain(id.chars()).any(char::is_control) {
        return None;
    }
    Some(format!(
        "santi --base-url {} {operation} {}",
        quote(base),
        quote(id)
    ))
}

fn quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials() {
        for base in [
            "https://user:password@service.invalid",
            "https://service.invalid?token=secret",
            "https://service.invalid#secret",
            "file:///secret",
            "not a URL",
        ] {
            let guidance = inspect(base, "ss_test", Some("inbox_test"));
            assert!(!guidance.contains("password"));
            assert!(!guidance.contains("secret"));
            assert!(!guidance.contains("`santi"));
            assert!(guidance.contains("credential-free"));
            assert!(!drive(base, "ss_test").contains("`santi"));
        }
    }

    #[test]
    fn quoted() {
        let command = command(
            "https://service.invalid/api",
            "receipt",
            "inbox '$(printf expanded)'",
        )
        .unwrap();
        let result = std::process::Command::new("sh")
            .args(["-c", &format!("set -- {command}; printf '%s\n' \"$@\"")])
            .output()
            .unwrap();
        assert!(result.status.success());
        let stdout = String::from_utf8(result.stdout).unwrap();
        assert_eq!(
            stdout.lines().collect::<Vec<_>>(),
            [
                "santi",
                "--base-url",
                "https://service.invalid/api",
                "receipt",
                "inbox '$(printf expanded)'"
            ]
        );
    }
}
