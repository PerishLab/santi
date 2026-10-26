use std::collections::BTreeSet;
use std::{fs, path::Path, path::PathBuf};

use super::Service;
use crate::{object, strand};

impl Service {
    pub async fn fork(&self, parent: &str) -> Result<strand::Forked, String> {
        let parent = self
            .store
            .strand(parent)
            .await?
            .ok_or_else(|| "parent strand not found".to_string())?;
        let fork = parent.next - 1;
        let child_tag = crate::tag("ss");
        let child = self
            .store
            .fork(santi_estate::ForkDraft {
                tag: &child_tag,
                parent: &parent.id,
                at: fork,
                created: &crate::now(),
            })
            .await?;
        if let Err(error) = self.bequeath(&parent.id, &child.id) {
            let nursery = self.strandhome(&child.id);
            if let Some(child_root) = nursery.parent() {
                let _ = fs::remove_dir_all(child_root);
            } else {
                let _ = fs::remove_dir_all(&nursery);
            }
            let _ = self.store.discard_fork(&child.id).await;
            return Err(format!("fork workspace sync failed: {error}"));
        }
        Ok(strand::Forked { strand: child })
    }

    fn bequeath(&self, parent: &str, child_strand_id: &str) -> Result<(), String> {
        let parent = self.strandhome(parent);
        let dir = self.strandhome(child_strand_id);
        if dir.exists() {
            return Err(format!(
                "child strand workspace already exists: {}",
                dir.display()
            ));
        }
        if !parent.exists() {
            fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
            return Ok(());
        }
        copied(&parent, &dir)
    }
}

fn copied(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|error| error.to_string())?;
    for entry in fs::read_dir(src).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        let src = entry.path();
        let dst = dst.join(entry.file_name());
        if kind.is_dir() {
            copied(&src, &dst)?;
        } else if kind.is_file() {
            fs::copy(&src, &dst).map_err(|error| error.to_string())?;
        } else if kind.is_symlink() {
            return Err(format!(
                "cannot copy symlink in strand workspace: {}",
                src.display()
            ));
        }
    }
    Ok(())
}

impl Service {
    pub async fn end_strand(&self, tag: &str) -> Result<(), String> {
        let strand = self
            .store
            .strand(tag)
            .await?
            .ok_or_else(|| "strand not found".to_string())?;
        let jobs = self.store.end_strand(tag).await?;
        let bucket = object::Bucket::new(&strand.soul, tag)?;
        let mut faults = Vec::new();
        if let Some(home) = self.strandhome(tag).parent() {
            faults.extend(remove(home).err());
        }
        faults.extend(self.objects().clear(&bucket).err());
        for key in jobs {
            faults.extend(remove(&self.runtime().join("jobs").join(key)).err());
        }
        if !faults.is_empty() {
            eprintln!(
                "santi: strand {tag} bytes left for the start sweep: {}",
                faults.join("; ")
            );
        }
        Ok(())
    }

    pub(in crate::service) async fn tidy(&self) -> Result<(), String> {
        let live = self
            .store
            .strands()
            .await?
            .into_iter()
            .map(|strand| strand.id)
            .collect::<BTreeSet<_>>();
        for home in dirs(&self.runtime().join("strands"))? {
            if !named(&home).is_none_or(|name| live.contains(name)) {
                remove(&home)?;
            }
        }
        let objects = self.objects();
        for bucket in objects.buckets()? {
            if live.contains(&bucket.strand) {
                self.unrecorded(&objects, &bucket).await?;
            } else {
                objects.clear(&bucket)?;
            }
        }
        for job in dirs(&self.runtime().join("jobs"))? {
            let Some(key) = named(&job).filter(|key| !key.starts_with('.')) else {
                continue;
            };
            if !self.store.retained_job(key).await? {
                remove(&job)?;
            }
        }
        Ok(())
    }

    async fn unrecorded(
        &self,
        objects: &object::Store,
        bucket: &object::Bucket,
    ) -> Result<(), String> {
        let recorded = self
            .store
            .materials(&bucket.strand)
            .await?
            .into_iter()
            .map(|material| material.key)
            .collect::<BTreeSet<_>>();
        for meta in objects.list(bucket, None)? {
            if !recorded.contains(&meta.uri.key) {
                objects.delete(&meta.uri)?;
            }
        }
        Ok(())
    }
}

fn named(path: &Path) -> Option<&str> {
    path.file_name()?.to_str()
}

fn dirs(root: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error.to_string()),
    };
    let mut dirs = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        if entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_dir()
        {
            dirs.push(entry.path());
        }
    }
    Ok(dirs)
}

fn remove(path: &Path) -> Result<(), String> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}
