mod api;
mod bucket;
mod compact;
mod downstream;
mod environment;
mod fork;
mod publication;
mod slots;
pub(in crate::service) mod wake;

use super::{Service, error};
pub use api::Admission;
