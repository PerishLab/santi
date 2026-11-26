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
    Creation, Environs, Jobs, Problem, Resolution, Window, drive, effect, errors, health, incident,
    openapi, retire, send, serve, settle, stranded,
};
