use santi_core::render;
use santi_core::{Fault, Health, environ, job, soul::Soul, strand::Strand, webhook};
use utoipa::{
    OpenApi,
    openapi::{RefOr, schema::Schema},
};

mod descriptions;
mod inference;
mod security;
use descriptions::{COMPONENTS, PROPERTIES};
use santi_core::{material, soul, strand, stream};
use security::Security;

#[derive(OpenApi)]
#[openapi(
    info(version = "v1"),
    modifiers(&Security),
    paths(
        super::routes::health,
        super::routes::materials::catalog,
        super::routes::materials::soul::load,
        super::routes::materials::soul::drop,
        super::routes::materials::strand::load,
        super::routes::materials::strand::drop,
        super::routes::materials::soul::query,
        super::routes::materials::strand::query,
        super::routes::alarms::set,
        super::routes::alarms::list,
        super::routes::alarms::get,
        super::routes::alarms::end,
        super::routes::alarms::fire,
        super::routes::weave,
        super::routes::seat,
        super::routes::strands,
        super::routes::awaken,
        super::routes::souls,
        super::routes::soul,
        super::routes::inference::soul::read,
        super::routes::inference::soul::declare,
        super::routes::inference::sources::list,
        super::routes::inference::sources::read,
        super::routes::inference::sources::explain,
        super::routes::inference::changes::register,
        super::routes::inference::changes::amend,
        super::routes::inference::changes::withdraw,

        super::routes::running,
        super::routes::owned,
        super::routes::declare,
        super::routes::revoke,
        super::routes::subscribe,
        super::routes::webhooks,
        super::jobs::create,
        super::jobs::query::query,
        super::jobs::list,
        super::jobs::get,
        super::jobs::cancel,
        super::jobs::logs,
        super::jobs::acknowledge,
        super::ingress::receive,
        super::routes::strand,
        super::routes::inference::strand::read,
        super::routes::inference::strand::declare,
        super::routes::retire,
        super::routes::retirement,
        super::routes::scoped,
        super::routes::assign,
        super::routes::unassign,
        super::routes::messages,
        super::routes::render,
        super::routes::send,
        super::routes::drive,
        super::routes::fork,
        super::routes::exec,
        super::routes::page,
        super::routes::audit,
        super::errors::stranded,
        super::errors::errors,
        super::errors::incident,
        super::routes::receipt,
        super::ingress::acknowledgement,
        super::ingress::feedback::routes::inspect,
        super::ingress::feedback::routes::reply,
        super::ingress::feedback::routes::resolve,
        super::ingress::confirmed,
        super::effects::effect,
        super::effects::trail,
        super::effects::settle,
        super::routes::active::activity::snapshot,
        super::routes::active::activity::events,
        super::sse::transitions,
        super::sse::turns,
        super::routes::snapshot,
        super::routes::tail,
        super::routes::events,
        super::routes::show,
        super::routes::stop,
        super::routes::enroll,
        super::routes::downstreams,
        super::routes::ingest,
        crate::bucket::fetch
    ),
    components(schemas(
        strand::Created,
        strand::Retirement,
        santi_core::sources::Kind,
        santi_core::inference::Declaration,
        santi_core::inference::Patch,
        santi_core::inference::Origin,
        santi_core::inference::Source,
        santi_core::inference::Label,
        santi_core::inference::Route,
        santi_core::inference::Effective,
        santi_core::inference::Reading,
        santi_core::inference::Draft,
        santi_core::alarm::Alarm,
        santi_core::alarm::Draft,
        santi_core::alarm::Projection,
        santi_core::alarm::View,
        santi_core::alarm::Occurrence,
        santi_core::event::Event,
        santi_core::event::Batch,
        santi_core::downstream::Credential,
        santi_core::downstream::Draft,
        santi_core::ingest::Request,
        strand::Forked,
        strand::Fork,
        strand::Workspace,
        soul::Draft,
        environ::Scope,
        environ::Variable,
        environ::Draft,
        webhook::Draft,
        webhook::Subscription,
        super::jobs::Creation,
        santi_core::job::Request,
        job::Accepted,
        job::Job,
        job::Query,
        job::Rows,
        job::Count,
        job::Origin,
        job::State,
        job::Stream,
        job::Log,
        Fault,
        Health,
        render::Request,
        material::View,
        material::Package,
        strand::Post,
        strand::Posted,
        santi_core::drive::Response,
        santi_core::drive::State,
        santi_core::ingest::Receipt,
        santi_core::receipt::Acknowledgement,
        santi_core::receipt::Confirmation,
        santi_core::receipt::State,
        santi_core::receipt::Status,
        santi_core::receipt::Transition,
        santi_core::effect::State,
        santi_core::effect::Status,
        santi_core::effect::Outcome,
        santi_core::trace::Record,
        santi_core::trace::Tag,
        super::effects::Resolution,
        strand::Detail,
        render::View,
        stream::Snapshot,
        stream::Order,
        stream::Record,
        stream::Tail,
        Soul,
        santi_core::service::interrupt::Projection,
        santi_core::service::interrupt::Entry,
        santi_core::service::interrupt::Strand,
        Strand,
        santi_core::message::Role,
        santi_core::compact::Compact,
        santi_core::compact::Capsule,
        santi_core::compact::Exec,
        santi_core::compact::Report,
        santi_core::compact::Range,
        santi_core::compact::Entry,
        santi_core::compact::Page,
        santi_core::budget::Cap,
        santi_core::budget::Estimate,
        santi_core::Category,
        santi_core::Exposure,
        santi_core::Incident,
        santi_core::Retry,
        santi_core::Scope,
        santi_core::Severity,
        santi_core::Source,
        santi_core::Transition,
        santi_core::Kind,
        santi_core::Status,
        santi_core::budget::Snapshot,
        santi_core::strand::Target,
        santi_core::message::Message,
        santi_core::message::Content,
        santi_core::message::Part,
        santi_core::message::State,
        santi_core::render::Kind,
        santi_core::render::Updated,
        santi_core::effect::Effect,
        santi_core::message::Placed,
        santi_core::message::Relation,
        santi_core::thinking::Span,
        santi_core::thinking::State,
        santi_core::tool::Call,
        santi_core::tool::Reply,
        santi_core::turn::Turn,
        santi_core::turn::activity::Snapshot,
        santi_core::turn::activity::Current,
        santi_core::turn::activity::Phase,
        santi_core::turn::Activity,
        santi_core::turn::Motion,
        santi_core::turn::Status,
        santi_core::turn::Trigger,
        santi_core::turn::Cause,
        santi_core::turn::Stop
    ))
)]
struct Document;

pub(super) fn document() -> utoipa::openapi::OpenApi {
    let mut document = Document::openapi();
    if let Some(components) = document.components.as_mut() {
        inference::snapshots(components);
    }
    for (schema, description) in COMPONENTS {
        component(&mut document, schema, description);
    }
    for (schema, name, description) in PROPERTIES {
        property(&mut document, schema, name, description);
    }
    document
}

fn component(document: &mut utoipa::openapi::OpenApi, name: &str, description: &str) {
    let Some(schema) = document
        .components
        .as_mut()
        .and_then(|components| components.schemas.get_mut(name))
    else {
        return;
    };
    describe(schema, description);
}

fn property(document: &mut utoipa::openapi::OpenApi, schema: &str, name: &str, description: &str) {
    let Some(RefOr::T(Schema::Object(schema))) = document
        .components
        .as_mut()
        .and_then(|components| components.schemas.get_mut(schema))
    else {
        return;
    };
    let Some(property) = schema.properties.get_mut(name) else {
        return;
    };
    describe(property, description);
}

fn describe(schema: &mut RefOr<Schema>, description: &str) {
    match schema {
        RefOr::Ref(reference) => reference.description = description.into(),
        RefOr::T(Schema::Object(object)) => object.description = Some(description.into()),
        RefOr::T(Schema::Array(array)) => array.description = Some(description.into()),
        RefOr::T(Schema::OneOf(one_of)) => {
            if let Some(item) = one_of
                .items
                .iter_mut()
                .find(|item| matches!(item, RefOr::Ref(_)))
            {
                describe(item, description);
            }
        }
        RefOr::T(Schema::AllOf(all_of)) => all_of.description = Some(description.into()),
        RefOr::T(Schema::AnyOf(any_of)) => any_of.description = Some(description.into()),
        _ => {}
    }
}
