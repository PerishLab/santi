use serde_json::Value;

pub(super) fn text(input: &str) -> (String, usize) {
    if let Ok(mut value) = serde_json::from_str::<Value>(input) {
        let count = json(&mut value);
        if count > 0 {
            return (
                serde_json::to_string(&value).expect("sanitized JSON must serialize"),
                count,
            );
        }
    }
    lines(input)
}

fn json(value: &mut Value) -> usize {
    match value {
        Value::Array(values) => values.iter_mut().map(json).sum(),
        Value::Object(fields) => fields
            .iter_mut()
            .map(|(key, value)| {
                if secret(key) {
                    *value = Value::String("[redacted]".to_string());
                    1
                } else {
                    json(value)
                }
            })
            .sum(),
        _ => 0,
    }
}

fn lines(input: &str) -> (String, usize) {
    let mut output = String::with_capacity(input.len());
    let mut count = 0;
    for line in input.split_inclusive('\n') {
        let (body, ending) = line.strip_suffix("\r\n").map_or_else(
            || {
                line.strip_suffix('\n')
                    .map_or((line, ""), |body| (body, "\n"))
            },
            |body| (body, "\r\n"),
        );
        if let Some((prefix, delimiter)) = assignment(body) {
            output.push_str(prefix);
            output.push(delimiter);
            output.push_str(" [redacted]");
            count += 1;
        } else {
            output.push_str(body);
        }
        output.push_str(ending);
    }
    (output, count)
}

fn assignment(line: &str) -> Option<(&str, char)> {
    let offset = line.find(['=', ':'])?;
    let (prefix, suffix) = line.split_at(offset);
    secret(prefix).then(|| (prefix, suffix.chars().next().expect("delimiter exists")))
}

fn secret(key: &str) -> bool {
    let normalized: String = key
        .trim()
        .trim_matches(['\'', '"'])
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect();
    matches!(
        normalized.as_str(),
        "token"
            | "accesstoken"
            | "refreshtoken"
            | "authtoken"
            | "apikey"
            | "secret"
            | "clientsecret"
            | "password"
            | "credential"
            | "privatekey"
            | "authorization"
    )
}
