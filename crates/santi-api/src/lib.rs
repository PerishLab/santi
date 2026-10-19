pub mod adaptor;
pub use adaptor as webhook;
pub mod config;
pub mod jobs;
pub mod ops;
pub mod provider;
pub mod runtime;

mod bucket;
mod server;

pub use santi_core::service::RETENTION;
pub use server::{
    ApiError, CreateJobRequest, Environs, Jobs, ResolveEffectRequest,
    drive as drive_strand_handler, effect as effect_status_handler, export_openapi_json,
    health as health_handler, send as send_strand_handler, serve, settle as resolve_effect_handler,
};
