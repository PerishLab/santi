use super::{env, trimmed};

pub(super) fn capacity(
    bytes: Option<usize>,
    maintenance: Option<usize>,
    provider: &str,
) -> Result<usize, String> {
    let bytes = positive(bytes, provider, "bytes")?;
    let ceiling = bytes.saturating_add(bytes / 2);
    let maintenance = maintenance.unwrap_or(ceiling).min(ceiling);
    if maintenance <= bytes {
        return Err(format!(
            "provider {provider} maintenance_bytes must exceed bytes"
        ));
    }
    Ok(maintenance)
}

pub(super) fn required(
    value: &Option<String>,
    provider: &str,
    field: &str,
) -> Result<String, String> {
    optional(value, provider, field)?
        .ok_or_else(|| format!("provider {provider} field {field} is required"))
}

pub(super) fn positive(value: Option<usize>, provider: &str, field: &str) -> Result<usize, String> {
    let value = value.ok_or_else(|| format!("provider {provider} field {field} is required"))?;
    if value == 0 {
        return Err(format!(
            "provider {provider} field {field} must be greater than zero"
        ));
    }
    Ok(value)
}

pub(super) fn optional(
    value: &Option<String>,
    provider: &str,
    field: &str,
) -> Result<Option<String>, String> {
    let Some(raw) = value.as_deref().and_then(trimmed) else {
        return Ok(None);
    };
    let Some(var) = raw.strip_prefix("env://") else {
        return Ok(Some(raw));
    };
    let var = var.trim();
    if var.is_empty() {
        return Err(format!(
            "provider {provider} field {field}: env:// reference is missing a variable name"
        ));
    }
    env(var).map(Some).ok_or_else(|| {
        format!("provider {provider} field {field} references env://{var}, which is unset or empty")
    })
}
