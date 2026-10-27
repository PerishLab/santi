use super::*;
use sha2::{Digest, Sha256};

impl Writer<'_, '_> {
    pub(super) async fn journal(
        &mut self,
        owner: &Owner,
        record: Record<'_>,
    ) -> Result<i64, Error> {
        let key = owner.key.to_string();
        let previous = record.previous.map(|v| v.to_string());
        let current = record.current.map(|v| v.to_string());
        let mut fields = vec![
            ("tag", record.operation),
            ("action", record.action),
            ("reference", record.reference),
            ("digest", record.digest),
            ("created", record.created),
            ("owner", key.as_str()),
        ];
        if let Some(value) = previous.as_deref() {
            fields.push(("previous", value));
        }
        if let Some(value) = current.as_deref() {
            fields.push(("current", value));
        }
        self.0.put(owner.journal(), &fields).await
    }

    pub(super) async fn inherit(
        &mut self,
        parent: i64,
        child: i64,
        created: &str,
    ) -> Result<(), Error> {
        let row = self.row("Strand", child).await?;
        let owner = Owner {
            scope: Scope::Strand,
            key: child,
            soul: int(&row, "soul")?,
            tag: text(&row, "tag")?,
        };
        let rows = self
            .0
            .ask(
                &form("Local")
                    .when("owner", Op::Eq, &parent.to_string())
                    .order("reference", Rank::Asc),
            )
            .await?;
        for row in rows.rows() {
            let reference = text(row, "reference")?;
            let revision = int(row, "revision")?;
            let digest = format!(
                "{:x}",
                Sha256::digest(format!("{parent}:{child}:{revision}").as_bytes())
            );
            let journal = self
                .journal(
                    &owner,
                    Record {
                        operation: &santi_model::tag("mo"),
                        action: "fork",
                        reference: &reference,
                        digest: &digest,
                        previous: None,
                        current: Some(revision),
                        created,
                    },
                )
                .await?;
            self.upsert(
                &owner,
                Entry {
                    reference: &reference,
                    revision,
                    provenance: &text(row, "provenance")?,
                    change: journal,
                    created,
                },
            )
            .await?;
        }
        Ok(())
    }
}

pub(super) struct Entry<'a> {
    pub(super) reference: &'a str,
    pub(super) revision: i64,
    pub(super) provenance: &'a str,
    pub(super) change: i64,
    pub(super) created: &'a str,
}

pub(super) struct Record<'a> {
    pub(super) operation: &'a str,
    pub(super) action: &'a str,
    pub(super) reference: &'a str,
    pub(super) digest: &'a str,
    pub(super) previous: Option<i64>,
    pub(super) current: Option<i64>,
    pub(super) created: &'a str,
}
