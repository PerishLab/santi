use super::super::Request;

pub(super) async fn listing(request: &Request<'_>, kind: &str) -> String {
    let path = format!("/api/v1/{kind}");
    let Ok(value) = request.get(&path).await else {
        return format!("{kind} unavailable");
    };
    let Some(held) = value.as_array() else {
        return format!("{kind} returned no list");
    };
    let names = crate::config::names().alias;
    let mut out = vec![format!("{kind}:")];
    for item in held.iter().take(24) {
        let Some(id) = item.get("id").and_then(serde_json::Value::as_str) else {
            continue;
        };
        let called = item
            .get("label")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
            .or_else(|| names.get(id).cloned())
            .unwrap_or_default();
        out.push(format!("  {id}  {called}"));
    }
    out.join("\n")
}

pub(super) fn renamed(id: &str, name: &str) -> String {
    let saved = match crate::config::rename(id, name) {
        Ok(path) => path,
        Err(error) => return format!("alias not saved: {error:#}"),
    };
    let where_ = saved.display();
    if name.is_empty() {
        format!("alias cleared; {where_} updated")
    } else {
        format!("alias set to {name}; {where_} updated")
    }
}
