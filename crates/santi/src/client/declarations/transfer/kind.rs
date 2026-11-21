use anyhow::{Context, Result, ensure};
use serde_json::Value;

pub(super) const ORDER: [&str; 8] = [
    "provider",
    "family",
    "scenario",
    "access",
    "alias",
    "offering",
    "candidate",
    "preference",
];

pub(super) type Key = (usize, Vec<String>);

pub(super) fn rank(row: &Value) -> Result<usize> {
    ORDER
        .iter()
        .position(|k| row["kind"].as_str() == Some(*k))
        .context("unknown catalog kind")
}

pub(super) fn name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 120
        && s.chars()
            .all(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

pub(super) fn text(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= 8192
}
pub(super) fn model(s: &str) -> bool {
    text(s) && s.trim() == s && !s.chars().any(char::is_control)
}

pub(super) struct Kind<'a>(pub(super) &'a str);

impl Kind<'_> {
    pub(super) fn fields(self) -> &'static [&'static str] {
        match self.0 {
            "provider" | "family" => &["name"],
            "scenario" => &["name", "criterion"],
            "access" => &["name", "protocol", "endpoint", "credential", "provider"],
            "alias" => &["name", "access"],
            "offering" => &["access", "model", "family"],
            "candidate" => &["offering", "scenario", "evidence"],
            "preference" => &["candidate"],
            _ => unreachable!(),
        }
    }

    pub(super) fn refs(self) -> &'static [&'static str] {
        match self.0 {
            "access" => &["provider"],
            "alias" => &["access"],
            "offering" => &["access", "family"],
            "candidate" => &["offering", "scenario"],
            "preference" => &["candidate"],
            _ => &[],
        }
    }

    pub(super) fn parts(self, v: &Value) -> Result<Vec<String>> {
        let vals = match self.0 {
            "offering" | "candidate" => v
                .as_array()
                .context("expected a portable reference tuple")?
                .as_slice(),
            _ => std::slice::from_ref(v),
        };
        let arity = match self.0 {
            "offering" => 2,
            "candidate" => 3,
            _ => 1,
        };
        ensure!(vals.len() == arity, "wrong reference tuple length");
        vals.iter()
            .enumerate()
            .map(|(i, v)| {
                let s = v.as_str().context("expected a name, not an estate ID")?;
                ensure!(if i == 1 { model(s) } else { name(s) }, "invalid reference");
                Ok(s.into())
            })
            .collect()
    }

    pub(super) fn reference(self, v: &Value) -> Result<Key> {
        Ok((
            ORDER.iter().position(|k| *k == self.0).unwrap(),
            self.parts(v)?,
        ))
    }
}
