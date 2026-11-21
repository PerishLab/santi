pub fn word(value: &str) -> bool {
    !value.is_empty() && value.len() <= 120 && value.bytes().all(|b| b.is_ascii_lowercase())
}

pub fn scenario(value: &str) -> bool {
    value.len() <= 120 && value.split('.').count() <= 3 && value.split('.').all(word)
}

pub fn path(value: &str) -> bool {
    let Some((provider, rest)) = value.split_once('/') else {
        return false;
    };
    let Some((family, scene)) = rest.split_once('/') else {
        return false;
    };
    word(provider) && word(family) && scenario(scene)
}
