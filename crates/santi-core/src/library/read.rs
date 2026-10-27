use std::{
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};

pub(super) const BYTES: usize = 262_144;

pub(super) fn name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 96
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(
            "material name must contain 1..96 ASCII letters, digits, underscores or hyphens".into(),
        );
    }
    Ok(())
}

pub(super) fn relative(value: &str) -> Result<(), String> {
    if value.is_empty()
        || !Path::new(value)
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err("material blob requires a relative path without traversal".into());
    }
    Ok(())
}

pub(super) struct Root(PathBuf);

impl Root {
    pub(super) fn new(path: &Path) -> Result<Self, String> {
        let root =
            fs::canonicalize(path).map_err(|e| format!("material source unavailable: {e}"))?;
        if !root.is_dir() {
            return Err("material source is not a directory".into());
        }
        Ok(Self(root))
    }

    pub(super) fn directory(&self, name: &str) -> Result<Self, String> {
        let path = self.within(name)?;
        if !path.is_dir() {
            return Err("material directory not found".into());
        }
        Ok(Self(path))
    }

    fn within(&self, value: &str) -> Result<PathBuf, String> {
        relative(value)?;
        let path = fs::canonicalize(self.0.join(value))
            .map_err(|e| format!("material source unreadable: {e}"))?;
        if !path.starts_with(&self.0) {
            return Err("material path escapes configured root".into());
        }
        Ok(path)
    }

    pub(super) fn exists(&self, value: &str) -> Result<bool, String> {
        relative(value)?;
        self.0.join(value).try_exists().map_err(|e| e.to_string())
    }

    pub(super) fn text(&self, value: &str) -> Result<String, String> {
        let path = self.within(value)?;
        if !fs::metadata(&path).map_err(|e| e.to_string())?.is_file() {
            return Err("material blob is not a regular file".into());
        }
        let mut options = fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NONBLOCK);
        }
        let file = options.open(path).map_err(|e| e.to_string())?;
        if !file.metadata().map_err(|e| e.to_string())?.is_file() {
            return Err("material blob is not a regular file".into());
        }
        let mut bytes = Vec::new();
        file.take((BYTES + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|e| e.to_string())?;
        if bytes.len() > BYTES {
            return Err("material blob exceeds 262144 bytes".into());
        }
        String::from_utf8(bytes).map_err(|_| "material blob must be UTF-8 text".into())
    }

    pub(super) fn children(&self) -> Result<Vec<String>, String> {
        let mut names = Vec::new();
        let entries =
            fs::read_dir(&self.0).map_err(|e| format!("material source unavailable: {e}"))?;
        for (count, entry) in entries.enumerate() {
            if count >= 512 {
                return Err("material source exceeds 512 entries".into());
            }
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| "material name must be UTF-8")?;
            if name.starts_with('.') || !entry.path().is_dir() {
                continue;
            }
            self::name(&name)?;
            names.push(name);
        }
        names.sort();
        Ok(names)
    }
}
