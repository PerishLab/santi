pub fn curbed(error: String, limit: Option<usize>) -> String {
    let Some(limit) = limit else {
        return error;
    };
    if error.len() <= limit {
        return error;
    }
    let mut end = limit;
    while end > 0 && !error.is_char_boundary(end) {
        end -= 1;
    }
    error[..end].to_string()
}

pub fn bounded(output: serde_json::Value, limit: Option<usize>) -> serde_json::Value {
    let Some(limit) = limit else {
        return output;
    };
    if output.to_string().len() <= limit {
        return output;
    }
    let compact = serde_json::json!({ "compact": output["compact"], "truncated": true });
    if compact.to_string().len() <= limit {
        return compact;
    }
    let truncated = serde_json::json!({ "truncated": true });
    if truncated.to_string().len() <= limit {
        return truncated;
    }
    if limit >= 4 {
        serde_json::Value::Null
    } else {
        serde_json::json!(0)
    }
}
