use keel::adapt::db::Sqlite;
use keel::{Core, Op, Row, Tx, form};

pub(in crate::store) fn column(kind: &str) -> Result<&'static str, keel::adapt::Error> {
    match kind {
        "message" => Ok("message"),
        "thinking" => Ok("thinking"),
        "tool_call" => Ok("call"),
        "tool_result" => Ok("result"),
        kind => Err(keel::adapt::Error::Adapt(format!(
            "unknown strand target {kind}"
        ))),
    }
}

pub(in crate::store) fn unit(kind: &str) -> Result<&'static str, keel::adapt::Error> {
    match kind {
        "message" => Ok("Message"),
        "thinking" => Ok("ThinkingSpan"),
        "tool_call" => Ok("ToolCall"),
        "tool_result" => Ok("ToolResult"),
        kind => Err(keel::adapt::Error::Adapt(format!(
            "unknown strand target {kind}"
        ))),
    }
}

pub(in crate::store) async fn target(core: &Core<Sqlite>, entry: &Row) -> Result<Row, String> {
    let kind = super::read::text(entry, "target_type")?;
    let unit = unit(kind).map_err(|error| error.to_string())?;
    let key = super::read::int(entry, column(kind).map_err(|error| error.to_string())?)?;
    super::read::one(core, unit, "id", &key.to_string())
        .await?
        .ok_or_else(|| format!("{unit} {key} missing"))
}

pub(in crate::store) async fn aim(
    tx: &mut Tx<'_, Sqlite>,
    entry: &Row,
) -> Result<Row, keel::adapt::Error> {
    let kind = entry
        .text("target_type")
        .ok_or_else(|| keel::adapt::Error::Adapt("entry target type missing".into()))?;
    let unit = unit(kind)?;
    let key = entry
        .int(column(kind)?)
        .ok_or_else(|| keel::adapt::Error::Adapt("entry target missing".into()))?;
    tx.one(&form(unit).when("id", Op::Eq, &key.to_string()))
        .await?
        .ok_or_else(|| keel::adapt::Error::Missing(format!("{unit} {key}")))
}
