pub(super) fn replace(old: &[u8], begin: &str, end: &str, block: &str) -> Result<Vec<u8>, String> {
    let text = std::str::from_utf8(old)
        .map_err(|_| "alarm cron contains non-UTF8 content; left unchanged")?;
    let mut first = None;
    let mut last = None;
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        if line.trim_end_matches('\n') == begin {
            if first.is_some() || last.is_some() {
                return Err("alarm cron has ambiguous managed markers".into());
            }
            first = Some(offset);
        }
        if line.trim_end_matches('\n') == end {
            if first.is_none() || last.is_some() {
                return Err("alarm cron has ambiguous managed markers".into());
            }
            last = Some(offset + line.len());
        }
        offset += line.len();
    }
    match (first, last) {
        (Some(first), Some(last)) => {
            Ok(format!("{}{block}{}", &text[..first], &text[last..]).into_bytes())
        }
        (None, None) if block.is_empty() => Ok(old.to_vec()),
        (None, None) => {
            if !text.is_empty() && !text.ends_with('\n') {
                return Err("alarm cron lacks a final newline; left unchanged".into());
            }
            Ok(format!("{text}{block}").into_bytes())
        }
        _ => Err("alarm cron has incomplete managed markers".into()),
    }
}

pub(super) fn quote(value: &str) -> Result<String, String> {
    if value.contains(['\n', '\r', '%']) {
        return Err("alarm cron argument contains unsupported control characters".into());
    }
    Ok(format!("'{}'", value.replace('\'', "'\"'\"'")))
}

pub(super) fn timezone() -> String {
    if let Ok(path) = std::fs::read_link("/etc/localtime")
        && let Ok(zone) = path.strip_prefix("/usr/share/zoneinfo")
    {
        return zone.display().to_string();
    }
    std::fs::read_to_string("/etc/timezone")
        .map(|zone| zone.trim().to_string())
        .unwrap_or_else(|_| "host timezone (unknown)".into())
}
