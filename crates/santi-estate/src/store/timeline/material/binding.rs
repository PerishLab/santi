use crate::store::{Store, read};
use keel::adapt::{Error, db::Sqlite};
use keel::{Op, Rank, Row, Tx, form};
use santi_model::{environ::Scope, material};

#[path = "context.rs"]
mod context;
#[path = "journal.rs"]
mod journal;
#[path = "reader.rs"]
mod reader;
#[path = "writer.rs"]
mod writer;

pub struct Bind<'a> {
    pub scope: Scope,
    pub owner: &'a str,
    pub package: &'a material::Package,
    pub operation: &'a str,
    pub created: &'a str,
}

pub struct Unbind<'a> {
    pub scope: Scope,
    pub owner: &'a str,
    pub reference: &'a str,
    pub operation: &'a str,
    pub created: &'a str,
}

pub struct Capture<'a> {
    pub tag: &'a str,
    pub turn: &'a str,
    pub round: usize,
    pub selection: &'a material::Selection,
    pub prompt: &'a str,
    pub rendered: &'a str,
    pub components: &'a [material::Component],
    pub created: &'a str,
}

pub(super) struct Owner {
    scope: Scope,
    key: i64,
    soul: i64,
    tag: String,
}

impl Owner {
    fn unit(&self) -> &'static str {
        match self.scope {
            Scope::Soul => "Shared",
            Scope::Strand => "Local",
        }
    }
    fn journal(&self) -> &'static str {
        match self.scope {
            Scope::Soul => "Declaration",
            Scope::Strand => "Change",
        }
    }
}

pub(super) struct Writer<'a, 't>(&'a mut Tx<'t, Sqlite>);

pub(super) fn text(row: &Row, name: &str) -> Result<String, Error> {
    row.text(name)
        .map(str::to_owned)
        .ok_or_else(|| Error::Adapt(format!("material {name} missing")))
}

pub(super) fn int(row: &Row, name: &str) -> Result<i64, Error> {
    row.int(name)
        .ok_or_else(|| Error::Adapt(format!("material {name} missing")))
}

pub(super) fn decode<T: serde::de::DeserializeOwned>(value: &str) -> Result<T, Error> {
    serde_json::from_str(value).map_err(|e| Error::Adapt(e.to_string()))
}

pub(super) fn encode<T: serde::Serialize>(value: &T) -> Result<String, Error> {
    serde_json::to_string(value).map_err(|e| Error::Adapt(e.to_string()))
}

impl Store {
    pub async fn bindings(&self, scope: Scope, owner: &str) -> Result<material::Selection, String> {
        self.core
            .batch(async |tx| {
                let mut writer = Writer(tx);
                let owner = writer.owner(scope, owner).await?;
                writer.selection(&owner).await
            })
            .await
            .map_err(read::error)
    }

    pub async fn bind(&self, draft: Bind<'_>) -> Result<material::Change, String> {
        self.core
            .batch(async |tx| Writer(tx).bind(draft).await)
            .await
            .map_err(read::error)
    }

    pub async fn unbind(&self, draft: Unbind<'_>) -> Result<material::Change, String> {
        self.core
            .batch(async |tx| Writer(tx).unbind(draft).await)
            .await
            .map_err(read::error)
    }

    pub async fn operation(
        &self,
        scope: Scope,
        owner: &str,
        operation: &str,
    ) -> Result<Option<material::Change>, String> {
        self.core
            .batch(async |tx| {
                let mut writer = Writer(tx);
                let owner = writer.owner(scope, owner).await?;
                match writer.operation(&owner, operation).await? {
                    Some(row) => writer.change(&owner, &row).await.map(Some),
                    None => Ok(None),
                }
            })
            .await
            .map_err(read::error)
    }
}

pub(in crate::store) async fn inherit(
    tx: &mut Tx<'_, Sqlite>,
    parent: i64,
    child: i64,
    created: &str,
) -> Result<(), Error> {
    Writer(tx).inherit(parent, child, created).await
}
