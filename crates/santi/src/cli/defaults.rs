use anyhow::Result;

pub struct ClientDefaults {
    pub strand: Option<String>,
    pub soul: Option<String>,
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
