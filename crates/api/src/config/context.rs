use std::path::PathBuf;

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Origin {
    pub path: PathBuf,
    pub source: &'static str,
    pub present: bool,
}

impl Origin {
    pub fn new(path: PathBuf, argument: bool, environment: bool) -> Result<Self, String> {
        let path = std::path::absolute(path).map_err(|error| error.to_string())?;
        let present = path.is_file();
        let source = if argument {
            "argument"
        } else if environment {
            "SANTI_CONFIG"
        } else {
            "home"
        };
        Ok(Self {
            path,
            source,
            present,
        })
    }
}
