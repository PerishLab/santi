use super::read;
use keel::{
    Row, Tx,
    adapt::{Error, db::Sqlite},
};
use santi_model::inference::{Label, Route};

pub(super) fn decode(row: &Row) -> Result<Route, String> {
    let named = |id, name| -> Result<Label, String> {
        Ok(Label {
            id: read::int(row, id)?,
            name: read::text(row, name)?.into(),
        })
    };
    Ok(Route {
        provider: named("provider", "provider_name")?,
        access: named("access", "access_name")?,
        family: row
            .int("family")
            .map(|_| named("family", "family_name"))
            .transpose()?,
        scenario: row
            .int("scenario")
            .map(|_| named("scenario", "scenario_name"))
            .transpose()?,
        candidate: row.int("candidate"),
        offering: row.int("offering"),
        protocol: read::text(row, "protocol")?.into(),
        endpoint: read::text(row, "endpoint")?.into(),
        identity: read::text(row, "identity")?.into(),
        policy: read::text(row, "policy")?.into(),
        custody: read::text(row, "custody")?.into(),
        thinking: row.text("thinking").map(str::to_owned),
        replay: row.flag("replay").ok_or("missing replay condition")?,
    })
}

pub(super) fn verify(
    rows: &[santi_model::catalog::Record],
    selected: &santi_model::inference::Effective,
    route: &Route,
) -> Result<(), String> {
    use santi_model::catalog::{Definition as D, Kind, selection::Directory};
    let changed = "catalog changed before turn admission";
    let dir = Directory(rows);
    let D::Access {
        provider,
        name,
        protocol,
        endpoint,
        credential,
    } = dir.find(Kind::Access, route.access.id)?
    else {
        unreachable!()
    };
    if *provider != route.provider.id
        || name != &route.access.name
        || dir.find(Kind::Provider, *provider)?.name() != Some(route.provider.name.as_str())
    {
        return Err(changed.into());
    }
    if protocol.label() != route.protocol
        || endpoint != &route.endpoint
        || credential.strip_prefix("custody://") != Some(route.custody.as_str())
    {
        return Err(changed.into());
    }
    let family = route.family.as_ref().ok_or(changed)?;
    let scene = route.scenario.as_ref().ok_or(changed)?;
    let offering = route.offering.ok_or(changed)?;
    let candidate = route.candidate.ok_or(changed)?;
    let choice = dir.explain(&route.provider.name, &family.name, &scene.name)?;
    if selected.provider != format!("{}/{}/{}", route.provider.name, family.name, scene.name) {
        return Err(changed.into());
    }
    if choice.family != family.id
        || choice.scenario != scene.id
        || choice.preferred != Some(candidate)
    {
        return Err(changed.into());
    }
    if !matches!(dir.find(Kind::Candidate, candidate)?, D::Candidate { offering: id, .. } if *id == offering)
        || !matches!(dir.find(Kind::Offering, offering)?, D::Offering { access, model, family: id }
            if *access == route.access.id && *id == family.id && model == &selected.model)
    {
        return Err(changed.into());
    }
    Ok(())
}

pub(super) async fn record(
    tx: &mut Tx<'_, Sqlite>,
    turn: &str,
    route: &Route,
) -> Result<(), Error> {
    let mut cells = vec![
        ("turn", turn.to_owned()),
        ("provider", route.provider.id.to_string()),
        ("provider_name", route.provider.name.clone()),
        ("access", route.access.id.to_string()),
        ("access_name", route.access.name.clone()),
        ("protocol", route.protocol.clone()),
        ("identity", route.identity.clone()),
        ("policy", route.policy.clone()),
        ("endpoint", route.endpoint.clone()),
        ("custody", route.custody.clone()),
        ("replay", route.replay.to_string()),
    ];
    for (name, value) in [("candidate", route.candidate), ("offering", route.offering)] {
        if let Some(id) = value {
            cells.push((name, id.to_string()));
        }
    }
    for (id, name, source) in [
        ("family", "family_name", &route.family),
        ("scenario", "scenario_name", &route.scenario),
    ] {
        if let Some(source) = source {
            cells.push((id, source.id.to_string()));
            cells.push((name, source.name.clone()));
        }
    }
    if let Some(thinking) = &route.thinking {
        cells.push(("thinking", thinking.clone()));
    }
    let values: Vec<_> = cells.iter().map(|(k, v)| (*k, v.as_str())).collect();
    tx.put("Route", &values).await?;
    Ok(())
}
