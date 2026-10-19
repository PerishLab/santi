use crate::alarm;

pub(super) fn validate(draft: &alarm::Draft) -> Result<(), String> {
    if draft.name.is_empty()
        || draft.name.len() > 128
        || !draft
            .name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
    {
        return Err("alarm name needs 1..128 ASCII letters, digits, underscores or hyphens".into());
    }
    if draft.text.trim().is_empty() {
        return Err("alarm text must contain text".into());
    }
    Ok(())
}
