use std::path::PathBuf;

use serde::Deserialize;

pub(crate) mod headers;
mod profile;
mod resolved;
pub use profile::*;
