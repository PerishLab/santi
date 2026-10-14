use plumb::config::Cascade;

#[derive(Debug, Cascade)]
#[cascade(section)]
pub struct Memory {
    pub bytes: usize,
}

impl Default for Memory {
    fn default() -> Self {
        Self {
            bytes: santi_core::service::MEMORY,
        }
    }
}

impl Memory {
    pub fn capacity(&self) -> Result<usize, String> {
        if self.bytes == 0 {
            return Err("shared memory bytes must be greater than zero".into());
        }
        Ok(self.bytes)
    }
}
