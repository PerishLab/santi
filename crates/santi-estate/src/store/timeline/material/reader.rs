use super::*;
use std::collections::BTreeMap;

impl Writer<'_, '_> {
    pub(super) async fn row(&mut self, unit: &str, key: i64) -> Result<Row, Error> {
        self.0
            .one(&form(unit).when("id", Op::Eq, &key.to_string()))
            .await?
            .ok_or_else(|| Error::Missing(format!("material {unit} {key}")))
    }

    pub(super) async fn owner(&mut self, scope: Scope, tag: &str) -> Result<Owner, Error> {
        let key = read::need(self.0, scope.unit(), "tag", tag).await?;
        let soul = match scope {
            Scope::Soul => key,
            Scope::Strand => int(&self.row("Strand", key).await?, "soul")?,
        };
        Ok(Owner {
            scope,
            key,
            soul,
            tag: tag.into(),
        })
    }

    pub(super) async fn operation(
        &mut self,
        owner: &Owner,
        operation: &str,
    ) -> Result<Option<Row>, Error> {
        let row = self
            .0
            .one(&form(owner.journal()).when("tag", Op::Eq, operation))
            .await?;
        if let Some(row) = row.as_ref()
            && int(row, "owner")? != owner.key
        {
            return Err(Error::Adapt(
                "material operation conflicts with owner".into(),
            ));
        }
        Ok(row)
    }

    pub(super) async fn binding(
        &mut self,
        owner: &Owner,
        reference: &str,
    ) -> Result<Option<Row>, Error> {
        self.0
            .one(
                &form(owner.unit())
                    .when("owner", Op::Eq, &owner.key.to_string())
                    .when("reference", Op::Eq, reference),
            )
            .await
    }

    pub(super) async fn package(&mut self, key: i64) -> Result<material::Package, Error> {
        let row = self.row("Revision", key).await?;
        let material = decode(&text(&row, "record")?)?;
        let rows = self
            .0
            .ask(
                &form("Blob")
                    .when("revision", Op::Eq, &key.to_string())
                    .order("name", Rank::Asc),
            )
            .await?;
        let mut contents = Vec::new();
        for row in rows.rows() {
            contents.push(material::Content {
                name: text(row, "name")?,
                mime: text(row, "mime")?,
                text: text(row, "text")?,
            });
        }
        Ok(material::Package { material, contents })
    }

    pub(super) async fn projected(
        &mut self,
        owner: &Owner,
        row: &Row,
    ) -> Result<(material::Binding, material::Package), Error> {
        let mut package = self.package(int(row, "revision")?).await?;
        package.material.provenance = decode(&text(row, "provenance")?)?;
        let operation = text(
            &self.row(owner.journal(), int(row, "change")?).await?,
            "tag",
        )?;
        let binding = material::Binding {
            id: text(row, "tag")?,
            scope: owner.scope,
            owner: owner.tag.clone(),
            reference: text(row, "reference")?,
            revision: package.material.revision.clone(),
            provenance: package.material.provenance.clone(),
            operation,
            updated: text(row, "updated")?,
        };
        Ok((binding, package))
    }

    pub(super) async fn selection(&mut self, owner: &Owner) -> Result<material::Selection, Error> {
        let soul = text(&self.row("Soul", owner.soul).await?, "tag")?;
        let shared = Owner {
            scope: Scope::Soul,
            key: owner.soul,
            soul: owner.soul,
            tag: soul,
        };
        let mut pairs = self.layer(&shared).await?;
        if owner.scope == Scope::Strand {
            pairs.extend(self.layer(owner).await?);
        }
        let mut packages = BTreeMap::<String, material::Package>::new();
        let mut bindings = Vec::new();
        for (binding, package) in pairs {
            if let Some(old) = packages.get(&binding.reference)
                && old.material.revision != binding.revision
            {
                return Err(Error::Adapt(
                    "material binding conflicts with effective revision".into(),
                ));
            }
            packages.entry(binding.reference.clone()).or_insert(package);
            bindings.push(binding);
        }
        bindings.sort_by(|a, b| {
            (&a.reference, a.scope.encode(), &a.owner).cmp(&(
                &b.reference,
                b.scope.encode(),
                &b.owner,
            ))
        });
        Ok(material::Selection {
            scope: owner.scope,
            owner: owner.tag.clone(),
            bindings,
            packages: packages.into_values().collect(),
        })
    }

    async fn layer(
        &mut self,
        owner: &Owner,
    ) -> Result<Vec<(material::Binding, material::Package)>, Error> {
        let rows = self
            .0
            .ask(
                &form(owner.unit())
                    .when("owner", Op::Eq, &owner.key.to_string())
                    .order("reference", Rank::Asc),
            )
            .await?;
        let mut pairs = Vec::new();
        for row in rows.rows() {
            pairs.push(self.projected(owner, row).await?);
        }
        Ok(pairs)
    }

    pub(super) async fn digest(&mut self, key: Option<i64>) -> Result<Option<String>, Error> {
        match key {
            Some(key) => text(&self.row("Revision", key).await?, "digest").map(Some),
            None => Ok(None),
        }
    }

    pub(super) async fn change(
        &mut self,
        owner: &Owner,
        row: &Row,
    ) -> Result<material::Change, Error> {
        Ok(material::Change {
            id: text(row, "tag")?,
            scope: owner.scope,
            owner: owner.tag.clone(),
            action: text(row, "action")?,
            reference: text(row, "reference")?,
            previous: self.digest(row.int("previous")).await?,
            current: self.digest(row.int("current")).await?,
            created: text(row, "created")?,
        })
    }
}
