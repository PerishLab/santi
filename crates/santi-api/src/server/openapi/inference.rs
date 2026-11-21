use utoipa::PartialSchema;
use utoipa::openapi::{
    Components, Ref, RefOr, Schema,
    schema::{AdditionalProperties, ObjectBuilder, OneOfBuilder, Type},
};

fn nullable(name: &str) -> RefOr<Schema> {
    OneOfBuilder::new()
        .item(ObjectBuilder::new().schema_type(Type::Null))
        .item(Ref::from_schema_name(name))
        .into()
}

fn pair(left: &str, right: &str) -> RefOr<Schema> {
    OneOfBuilder::new()
        .item(Ref::from_schema_name(left))
        .item(Ref::from_schema_name(right))
        .into()
}

pub(super) fn snapshots(components: &mut Components) {
    let schemas = &mut components.schemas;
    for (key, selection, source) in [
        (
            "scenario",
            "inference.ScenarioSelection",
            "inference.ScenarioSource",
        ),
        (
            "provider",
            "inference.ProviderSelection",
            "inference.ProviderSource",
        ),
    ] {
        let object = || {
            ObjectBuilder::new()
                .required(key)
                .required("model")
                .additional_properties(Some(AdditionalProperties::FreeForm(false)))
        };
        let provenance = Ref::from_schema_name("inference.Origin");
        schemas.insert(
            source.into(),
            object()
                .property(key, provenance.clone())
                .property("model", provenance)
                .property("route", nullable("Route"))
                .property("reasoning_effort", nullable("inference.Origin"))
                .into(),
        );
        schemas.insert(
            selection.into(),
            object()
                .property(key, String::schema())
                .property("model", String::schema())
                .property("reasoning_effort", Option::<String>::schema())
                .required("source")
                .property("source", Ref::from_schema_name(source))
                .into(),
        );
    }
    schemas.insert(
        "inference.Effective".into(),
        pair("inference.ScenarioSelection", "inference.ProviderSelection"),
    );
    schemas.insert(
        "inference.Source".into(),
        pair("inference.ScenarioSource", "inference.ProviderSource"),
    );
    if let Some(RefOr::T(Schema::Object(reading))) = schemas.get_mut("inference.Reading") {
        reading
            .properties
            .insert("effective".into(), nullable("inference.ScenarioSelection"));
    }
}
