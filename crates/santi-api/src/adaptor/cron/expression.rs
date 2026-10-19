pub(super) fn validate(text: &str) -> Result<(), String> {
    if text.contains(['\n', '\r']) {
        return Err("alarm cron must be one five-field expression".into());
    }
    let fields = text.split_whitespace().collect::<Vec<_>>();
    if fields.len() != 5 {
        return Err("alarm cron needs five numeric fields".into());
    }
    for (field, range) in fields
        .iter()
        .zip([(0, 59), (0, 23), (1, 31), (1, 12), (0, 7)])
    {
        for part in field.split(',') {
            check(part, range)?;
        }
    }
    Ok(())
}

fn check(part: &str, range: (u8, u8)) -> Result<(), String> {
    let pieces = part.split('/').collect::<Vec<_>>();
    if pieces.len() > 2 {
        return Err("alarm cron has an invalid step".into());
    }
    if let Some(step) = pieces.get(1) {
        number(step, (1, range.1.max(1)))?;
    }
    if pieces[0] == "*" {
        return Ok(());
    }
    let bounds = pieces[0].split('-').collect::<Vec<_>>();
    match bounds.as_slice() {
        [single] => {
            number(single, range)?;
        }
        [first, last] => {
            if number(first, range)? > number(last, range)? {
                return Err("alarm cron range is reversed".into());
            }
        }
        _ => return Err("alarm cron has an invalid range".into()),
    }
    Ok(())
}

fn number(text: &str, range: (u8, u8)) -> Result<u8, String> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("alarm cron fields must be numeric".into());
    }
    let value = text
        .parse::<u8>()
        .map_err(|_| "alarm cron fields must be numeric".to_string())?;
    if value < range.0 || value > range.1 {
        return Err("alarm cron field is outside its range".into());
    }
    Ok(value)
}
