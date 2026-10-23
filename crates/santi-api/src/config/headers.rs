use santi_provider::{DENIED, RECEIPT};

pub(super) fn allowed(names: Option<&[String]>, provider: &str) -> Result<Vec<String>, String> {
    let Some(names) = names else {
        return Ok(RECEIPT.iter().map(|name| name.to_string()).collect());
    };
    let mut allowed = Vec::with_capacity(names.len());
    for name in names {
        let name = name.trim().to_ascii_lowercase();
        if name.is_empty() {
            return Err(format!(
                "provider {provider} field headers holds an empty name"
            ));
        }
        if DENIED.contains(&name.as_str()) {
            return Err(format!(
                "provider {provider} field headers may not record {name}"
            ));
        }
        if !allowed.contains(&name) {
            allowed.push(name);
        }
    }
    Ok(allowed)
}
