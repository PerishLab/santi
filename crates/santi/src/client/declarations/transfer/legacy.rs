use anyhow::{Context, Result, anyhow, ensure};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

use super::graph::Graph;
use super::kind::name;

#[derive(Deserialize)]
struct Legacy {
    providers: BTreeMap<String, Profile>,
    #[serde(default)]
    custody: BTreeMap<String, Binding>,
    provider: Option<String>,
    model: Option<String>,
    #[serde(default)]
    souls: BTreeMap<String, Soul>,
}

#[derive(Deserialize)]
struct Soul {
    inference: Option<Declaration>,
}
#[derive(Deserialize)]
struct Declaration {
    provider: Option<String>,
    model: Option<String>,
}
#[derive(Deserialize)]
struct Profile {
    kind: String,
    base_url: Option<String>,
    model: Option<String>,
}
#[derive(Deserialize)]
struct Binding {
    kind: String,
    base_url: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Mapping {
    profiles: BTreeMap<String, Entry>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    provider: String,
    access: String,
    custody: String,
    aliases: Vec<String>,
    #[serde(default, rename = "default_in_custody")]
    private: bool,
    offerings: BTreeMap<String, String>,
}

fn endpoint<'a>(kind: &str, url: Option<&'a str>) -> Result<&'a str> {
    url.filter(|s| !s.is_empty())
        .or_else(|| (kind == "openai_responses").then_some("https://api.openai.com/v1"))
        .context("profile requires an explicit public base_url")
}

fn covered(entries: &BTreeMap<String, Entry>, profile: &str, model: Option<&str>) -> Result<()> {
    ensure!(name(profile), "invalid legacy provider reference");
    let m = entries
        .get(profile)
        .with_context(|| format!("missing mapping for provider {profile}"))?;
    if let Some(public) = model.filter(|s| !s.is_empty()) {
        ensure!(
            m.offerings.contains_key(public),
            "profile {profile}: model not covered; supply an explicit public offering and family"
        );
    }
    Ok(())
}

pub(super) fn convert(source: &str, mapping: &str) -> Result<(Graph, Value)> {
    let legacy: Legacy =
        toml::from_str(source).map_err(|_| anyhow!("invalid legacy service TOML"))?;
    let Mapping { profiles } = toml::from_str(mapping).map_err(|_| anyhow!("invalid mapping: each profiles.NAME requires provider, access, custody, aliases and offerings"))?;
    let before: BTreeSet<_> = legacy.providers.keys().collect();
    let after: BTreeSet<_> = profiles.keys().collect();
    ensure!(
        before.iter().chain(&after).all(|k| name(k)),
        "invalid profile name"
    );
    ensure!(
        before == after,
        "profile coverage mismatch: missing {:?}, unknown {:?}",
        before.difference(&after).collect::<Vec<_>>(),
        after.difference(&before).collect::<Vec<_>>()
    );
    let mut graph = Graph::default();
    let mut coverage = Vec::new();
    let mut bindings = BTreeMap::new();
    for (old, p) in &legacy.providers {
        let m = &profiles[old];
        ensure!(
            name(&m.custody),
            "profile {old}: expected custody name, not a credential"
        );
        ensure!(
            &m.access == old || m.aliases.contains(old),
            "profile {old}: preserve the legacy name as access or alias"
        );
        if !m.private {
            covered(&profiles, old, p.model.as_deref())?;
        }
        let url = endpoint(&p.kind, p.base_url.as_deref())?;
        let bind = (p.kind.as_str(), url);
        ensure!(
            bindings
                .insert(&m.custody, bind)
                .is_none_or(|prev| prev == bind),
            "profile {old}: custody name bound to different protocol/endpoint"
        );
        if let Some(c) = legacy.custody.get(&m.custody) {
            ensure!(
                (c.kind.as_str(), endpoint(&c.kind, c.base_url.as_deref())?) == bind,
                "profile {old}: local custody binding mismatch"
            );
        }
        graph.insert(json!({"kind":"provider", "name":m.provider}))?;
        graph.insert(json!({"kind":"access", "name":m.access, "provider":m.provider, "protocol":p.kind, "endpoint":url, "credential":format!("custody://{}",m.custody)}))?;
        for alias in &m.aliases {
            graph.insert(json!({"kind":"alias", "name":alias, "access":m.access}))?;
        }
        for (model, family) in &m.offerings {
            ensure!(
                !model.starts_with("env://"),
                "profile {old}: supply a public model identifier"
            );
            graph.insert(json!({"kind":"family", "name":family}))?;
            graph.insert(
                json!({"kind":"offering", "access":m.access, "model":model, "family":family}),
            )?;
        }
        coverage.push(json!({"profile":old, "provider":m.provider, "access":m.access, "aliases":m.aliases, "offerings":m.offerings.len(), "default_in_custody":m.private, "custody_binding_in_input":legacy.custody.contains_key(&m.custody)}));
    }
    let default = legacy.provider.as_deref().unwrap_or("openai");
    if !profiles.is_empty() || legacy.provider.is_some() || legacy.model.is_some() {
        covered(&profiles, default, legacy.model.as_deref())?;
    }
    for d in legacy.souls.values().filter_map(|s| s.inference.as_ref()) {
        covered(
            &profiles,
            d.provider.as_deref().unwrap_or(default),
            d.model.as_deref(),
        )?;
    }
    graph.validate()?;
    Ok((graph, json!(coverage)))
}
