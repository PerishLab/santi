use super::Profile;

impl Profile {
    pub(crate) fn seal(&self) -> Option<&str> {
        let (Self::Gemini { seal, .. }
        | Self::Completions { seal, .. }
        | Self::Responses { seal, .. }) = self;
        seal.as_deref()
    }
}

impl std::fmt::Debug for Profile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Profile")
            .field("kind", &self.kind())
            .finish_non_exhaustive()
    }
}
