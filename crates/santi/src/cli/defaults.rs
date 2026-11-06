use crate::inference::Declaration;
use anyhow::Result;

pub struct ClientDefaults {
    pub strand: Option<String>,
    pub soul: Option<String>,
    pub held: Declaration,
}

impl ClientDefaults {
    pub fn resolve_strand(&self, explicit: Option<String>) -> Result<String> {
        explicit
            .or_else(|| self.strand.clone())
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty())
            .ok_or_else(|| {
                anyhow::anyhow!("no strand id: pass one or set --strand / SANTI_STRAND_ID")
            })
    }

    pub fn soul(&self) -> Option<&str> {
        self.soul
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
    }

    pub fn require(&self) -> Result<&str> {
        self.soul()
            .ok_or_else(|| anyhow::anyhow!("no soul id: pass --soul or set SANTI_SOUL_ID"))
    }
}

pub fn split_send_args(
    mut args: Vec<String>,
    defaults: &ClientDefaults,
) -> Result<(String, String)> {
    match args.len() {
        2 => {
            let text = args.pop().expect("len == 2");
            let id = args.pop().expect("len == 2");
            Ok((defaults.resolve_strand(Some(id))?, text))
        }
        1 => {
            let text = args.pop().expect("len == 1");
            Ok((defaults.resolve_strand(None)?, text))
        }
        _ => anyhow::bail!("send takes `<id> <text>` or `<text>`"),
    }
}
