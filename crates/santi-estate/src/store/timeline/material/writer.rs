use super::journal::{Entry, Record};
use super::*;
use sha2::{Digest, Sha256};

fn fingerprint(
    action: &str,
    owner: &Owner,
    package: Option<&material::Package>,
    reference: &str,
) -> String {
    let value = serde_json::json!({"action":action,"scope":owner.scope.encode(),"owner":owner.tag,
        "reference":reference,"revision":package.map(|p|&p.material.revision),"provenance":package.map(|p|&p.material.provenance)});
    format!("{:x}", Sha256::digest(value.to_string().as_bytes()))
}

fn operation(value: &str) -> Result<(), Error> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(Error::Adapt(
            "material operation requires 1..128 ASCII letters, digits, underscores or hyphens"
                .into(),
        ));
    }
    Ok(())
}

impl Writer<'_, '_> {
    pub(super) async fn bind(&mut self, draft: Bind<'_>) -> Result<material::Change, Error> {
        draft.package.verify().map_err(Error::Adapt)?;
        operation(draft.operation)?;
        let owner = self.owner(draft.scope, draft.owner).await?;
        let reference = &draft.package.material.reference;
        let digest = fingerprint("load", &owner, Some(draft.package), reference);
        if let Some(row) = self.operation(&owner, draft.operation).await? {
            return self.repeated(&owner, &row, &digest).await;
        }
        let revision = self
            .revision(owner.soul, draft.package, draft.created)
            .await?;
        self.compatible(&owner, reference, revision).await?;
        let previous = self
            .binding(&owner, reference)
            .await?
            .map(|row| int(&row, "revision"))
            .transpose()?;
        let journal = self
            .journal(
                &owner,
                Record {
                    operation: draft.operation,
                    action: "load",
                    reference,
                    digest: &digest,
                    previous,
                    current: Some(revision),
                    created: draft.created,
                },
            )
            .await?;
        self.upsert(
            &owner,
            Entry {
                reference,
                revision,
                provenance: &encode(&draft.package.material.provenance)?,
                change: journal,
                created: draft.created,
            },
        )
        .await?;
        let row = self.row(owner.journal(), journal).await?;
        self.change(&owner, &row).await
    }

    pub(super) async fn unbind(&mut self, draft: Unbind<'_>) -> Result<material::Change, Error> {
        operation(draft.operation)?;
        let owner = self.owner(draft.scope, draft.owner).await?;
        let digest = fingerprint("drop", &owner, None, draft.reference);
        if let Some(row) = self.operation(&owner, draft.operation).await? {
            return self.repeated(&owner, &row, &digest).await;
        }
        let old = self.binding(&owner, draft.reference).await?;
        let previous = old.as_ref().map(|row| int(row, "revision")).transpose()?;
        if let Some(row) = old {
            self.0.end(owner.unit(), row.key()).await?;
        }
        let journal = self
            .journal(
                &owner,
                Record {
                    operation: draft.operation,
                    action: "drop",
                    reference: draft.reference,
                    digest: &digest,
                    previous,
                    current: None,
                    created: draft.created,
                },
            )
            .await?;
        let row = self.row(owner.journal(), journal).await?;
        self.change(&owner, &row).await
    }

    async fn repeated(
        &mut self,
        owner: &Owner,
        row: &Row,
        digest: &str,
    ) -> Result<material::Change, Error> {
        if text(row, "digest")? != digest {
            return Err(Error::Adapt(
                "material operation conflicts with previous payload".into(),
            ));
        }
        self.change(owner, row).await
    }

    async fn revision(
        &mut self,
        soul: i64,
        package: &material::Package,
        created: &str,
    ) -> Result<i64, Error> {
        let key = format!(
            "{}@{}",
            package.material.reference, package.material.revision
        );
        if let Some(row) = self
            .0
            .one(
                &form("Revision")
                    .when("owner", Op::Eq, &soul.to_string())
                    .when("key", Op::Eq, &key),
            )
            .await?
        {
            return Ok(row.key());
        }
        let revision = self
            .0
            .put(
                "Revision",
                &[
                    ("tag", &santi_model::tag("mr")),
                    ("key", &key),
                    ("reference", &package.material.reference),
                    ("digest", &package.material.revision),
                    ("record", &encode(&package.material)?),
                    ("created", created),
                    ("owner", &soul.to_string()),
                ],
            )
            .await?;
        for content in &package.contents {
            let digest = format!("{:x}", Sha256::digest(content.text.as_bytes()));
            self.0
                .put(
                    "Blob",
                    &[
                        ("name", &content.name),
                        ("mime", &content.mime),
                        ("digest", &digest),
                        ("text", &content.text),
                        ("revision", &revision.to_string()),
                    ],
                )
                .await?;
        }
        Ok(revision)
    }

    async fn compatible(
        &mut self,
        owner: &Owner,
        reference: &str,
        revision: i64,
    ) -> Result<(), Error> {
        if owner.scope == Scope::Strand {
            let shared = Owner {
                scope: Scope::Soul,
                key: owner.soul,
                soul: owner.soul,
                tag: String::new(),
            };
            return self.agrees(&shared, reference, revision).await;
        }
        let strands = self
            .0
            .ask(&form("Strand").when("soul", Op::Eq, &owner.soul.to_string()))
            .await?;
        for row in strands.rows() {
            let local = Owner {
                scope: Scope::Strand,
                key: row.key(),
                soul: owner.soul,
                tag: text(row, "tag")?,
            };
            self.agrees(&local, reference, revision).await?;
        }
        Ok(())
    }

    async fn agrees(&mut self, owner: &Owner, reference: &str, revision: i64) -> Result<(), Error> {
        if let Some(row) = self.binding(owner, reference).await?
            && int(&row, "revision")? != revision
        {
            return Err(Error::Adapt(format!(
                "material binding conflicts: {reference} has another revision in {}",
                owner.scope.encode()
            )));
        }
        Ok(())
    }

    pub(super) async fn upsert(&mut self, owner: &Owner, entry: Entry<'_>) -> Result<(), Error> {
        let revision = entry.revision.to_string();
        let change = entry.change.to_string();
        let mut fields = vec![
            ("revision", revision.as_str()),
            ("provenance", entry.provenance),
            ("change", change.as_str()),
            ("updated", entry.created),
        ];
        match self.binding(owner, entry.reference).await? {
            Some(row) => self.0.set(owner.unit(), row.key(), &fields).await,
            None => {
                let tag = santi_model::tag("mb");
                let key = owner.key.to_string();
                fields.extend([
                    ("tag", tag.as_str()),
                    ("owner", key.as_str()),
                    ("reference", entry.reference),
                ]);
                self.0.put(owner.unit(), &fields).await.map(|_| ())
            }
        }
    }
}
