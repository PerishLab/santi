mod policy;
use super::Catalog;
use crate::config::Resolved;
use santi_core::inference::{Declaration, Defaults, Directory, Effective, Label, Origin, Route};
use santi_core::sources::{Definition as D, Kind, Record, selection};
use santi_provider::chat::completions::Replay;

impl Directory for Resolved {
    fn defaults(&self, _: &str) -> Result<Defaults, String> {
        Ok(Defaults {
            model: self.model().into(),
            effort: self.effort().map(str::to_owned),
        })
    }
}

fn named(dir: &selection::Directory<'_>, kind: Kind, id: i64) -> Result<Label, String> {
    let name = dir.find(kind, id)?.name().ok_or("expected named source")?;
    Ok(Label {
        id,
        name: name.into(),
    })
}

impl Catalog<'_> {
    fn transport(&self, route: &Route, model: &str) -> Result<(String, Resolved, String), String> {
        let profile = self
            .custody
            .get(&route.custody)
            .ok_or("unknown custody handle")?;
        let identity = match profile {
            crate::config::Profile::Gemini {
                identity: Some(name),
                ..
            }
            | crate::config::Profile::Completions {
                identity: Some(name),
                ..
            } => name.clone(),
            _ => format!("estate/{}/access/{}", route.provider.id, route.access.id),
        };
        let key = profile.seal();
        let profile = profile.resolve(&identity, model)?;
        let endpoint = match &profile {
            Resolved::Completions(p) => &p.endpoint,
            Resolved::Responses(p) => &p.endpoint,
            Resolved::Gemini(p) => &p.endpoint,
        };
        if profile.kind() != route.protocol || endpoint != &route.endpoint {
            return Err("access endpoint or protocol does not match operator custody".into());
        }
        let stamp = policy::stamp(&profile, key, route)?;
        Ok((identity, profile, stamp))
    }

    pub(super) fn bound(&self, route: &Route, model: &str) -> Result<Resolved, String> {
        let (identity, profile, stamp) = self.transport(route, model)?;
        if identity != route.identity || stamp != route.policy {
            return Err("admitted transport policy differs from operator custody".into());
        }
        Ok(profile)
    }

    pub(super) fn select(
        &self,
        layers: &[(&Declaration, Origin)],
        rows: &[Record],
    ) -> Result<Effective, String> {
        let (route, model) = Self::offered(layers, rows)?;
        self.admit(layers, route, model)
    }

    pub(crate) fn offered<'a>(
        layers: &[(&Declaration, Origin)],
        rows: &'a [Record],
    ) -> Result<(Route, &'a str), String> {
        for (declaration, _) in layers {
            declaration.validate()?;
        }
        let selector = layers
            .iter()
            .rev()
            .find_map(|(d, _)| d.scenario.as_deref())
            .ok_or("inference resolution requires a scenario")?;
        let parts: Vec<_> = selector.split('/').collect();
        let [provider, family, scenario] = parts.as_slice() else {
            return Err("expected provider/family/scenario".into());
        };
        let dir = selection::Directory(rows);
        let choice = dir.explain(provider, family, scenario)?;
        let candidate = choice
            .preferred
            .ok_or_else(|| format!("no preferred candidate for {selector}"))?;
        let D::Candidate { offering, .. } = dir.find(Kind::Candidate, candidate)? else {
            unreachable!()
        };
        let D::Offering { access, model, .. } = dir.find(Kind::Offering, *offering)? else {
            unreachable!()
        };
        let definition = dir.find(Kind::Access, *access)?;
        definition.validate()?;
        let D::Access {
            provider,
            protocol,
            endpoint,
            credential,
            ..
        } = definition
        else {
            unreachable!()
        };
        let route = Route {
            provider: named(&dir, Kind::Provider, *provider)?,
            access: named(&dir, Kind::Access, *access)?,
            family: Some(named(&dir, Kind::Family, choice.family)?),
            scenario: Some(named(&dir, Kind::Scenario, choice.scenario)?),
            offering: Some(*offering),
            candidate: Some(candidate),
            protocol: protocol.label().into(),
            endpoint: endpoint.clone(),
            custody: credential
                .strip_prefix("custody://")
                .ok_or("invalid custody handle")?
                .into(),
            identity: String::new(),
            policy: String::new(),
            replay: false,
            thinking: None,
        };
        Ok((route, model))
    }

    pub(crate) fn admit(
        &self,
        layers: &[(&Declaration, Origin)],
        mut route: Route,
        model: &str,
    ) -> Result<Effective, String> {
        let (identity, transport, stamp) = self.transport(&route, model)?;
        route.identity = identity;
        route.policy = stamp;
        if let Resolved::Completions(chat) = &transport {
            route.replay = chat.replay == Replay::History;
            route.thinking.clone_from(&chat.thinking);
        }
        let mut effective = Effective::resolve(layers, &transport)?;
        effective.source.model = Origin::Offering;
        effective.source.route = Some(Box::new(route));
        Ok(effective)
    }
}

#[cfg(test)]
pub(super) mod tests;
