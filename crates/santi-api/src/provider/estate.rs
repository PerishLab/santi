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
            reasoning_effort: self.effort().map(str::to_owned),
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
    fn transport(&self, route: &Route) -> Result<(String, Resolved, String), String> {
        let profile = self
            .custody
            .get(&route.custody)
            .ok_or("unknown custody handle")?;
        let identity = match profile {
            crate::config::Profile::Gemini {
                identity: Some(name),
                ..
            }
            | crate::config::Profile::ChatCompletions {
                identity: Some(name),
                ..
            } => name.clone(),
            _ => format!("estate/{}/access/{}", route.provider.id, route.access.id),
        };
        let key = profile.seal();
        let profile = profile.resolve(&identity)?;
        let endpoint = match &profile {
            Resolved::ChatCompletions(p) => &p.base_url,
            Resolved::OpenAiResponses(p) => &p.base_url,
            Resolved::Gemini(p) => &p.base_url,
        };
        if profile.kind() != route.protocol || endpoint != &route.endpoint {
            return Err("access endpoint or protocol does not match operator custody".into());
        }
        let stamp = policy::stamp(&profile, key, route)?;
        Ok((identity, profile, stamp))
    }

    pub(super) fn bound(&self, route: &Route) -> Result<Resolved, String> {
        let (identity, profile, stamp) = self.transport(route)?;
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
        let selector = layers
            .iter()
            .rev()
            .find_map(|(d, _)| d.provider.as_deref())
            .ok_or("inference resolution requires a provider")?;
        let dir = selection::Directory(rows);
        let mut family = None;
        let mut scene = None;
        let mut offering = None;
        let mut candidate = None;
        let mut offered = None;
        let access = if selector.contains('/') {
            let parts: Vec<_> = selector.split('/').collect();
            let [p, f, s] = parts.as_slice() else {
                return Err("expected provider/family/scenario".into());
            };
            let choice = dir.explain(p, f, s)?;
            let id = choice
                .preferred
                .ok_or_else(|| format!("no preferred candidate for {selector}"))?;
            let D::Candidate {
                offering: offer, ..
            } = dir.find(Kind::Candidate, id)?
            else {
                unreachable!()
            };
            let D::Offering { access, model, .. } = dir.find(Kind::Offering, *offer)? else {
                unreachable!()
            };
            family = Some(named(&dir, Kind::Family, choice.family)?);
            scene = Some(named(&dir, Kind::Scenario, choice.scenario)?);
            offering = Some(*offer);
            candidate = Some(id);
            offered = Some(model);
            *access
        } else {
            rows.iter()
                .find_map(|row| match &row.definition {
                    D::Access { name, .. } if name == selector => Some(row.id),
                    D::Alias { name, access } if name == selector => Some(*access),
                    _ => None,
                })
                .ok_or_else(|| format!("unknown access or alias {selector}"))?
        };
        let definition = dir.find(Kind::Access, access)?;
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
        let mut route = Route {
            provider: named(&dir, Kind::Provider, *provider)?,
            access: named(&dir, Kind::Access, access)?,
            family,
            scenario: scene,
            offering,
            candidate,
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
        let (identity, transport, stamp) = self.transport(&route)?;
        route.identity = identity;
        route.policy = stamp;
        if let Resolved::ChatCompletions(chat) = &transport {
            route.replay = chat.replay == Replay::History;
            route.thinking.clone_from(&chat.thinking);
        }
        let mut effective = Effective::resolve(layers, &transport)?;
        if let Some(model) = offered {
            effective.model.clone_from(model);
            effective.source.model = Origin::Offering;
        }
        if route.offering.is_none() {
            for row in rows {
                if let D::Offering {
                    access: a,
                    family,
                    model,
                } = &row.definition
                    && *a == access
                    && model == &effective.model
                {
                    route.offering = Some(row.id);
                    route.family = Some(named(&dir, Kind::Family, *family)?);
                    break;
                }
            }
        }
        effective.source.route = Some(Box::new(route));
        Ok(effective)
    }
}

#[cfg(test)]
mod tests;
