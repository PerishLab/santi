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

pub(super) fn requests(
    values: &std::collections::BTreeMap<String, String>,
    provider: &str,
) -> Result<std::collections::BTreeMap<String, String>, String> {
    compile(values).map_err(|error| format!("provider {provider}: {error}"))?;
    Ok(values.clone())
}

pub(crate) fn compile(
    values: &std::collections::BTreeMap<String, String>,
) -> Result<santi_provider::Headers, String> {
    if values.is_empty() {
        return Ok(Default::default());
    }
    let agent = plumb::agent!("SANTI", "santi-api")?;
    santi_provider::Headers::new(values, agent.value())
}
