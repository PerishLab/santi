use crate::store::{Store, read};
use keel::{
    Cell, Row, Tx,
    adapt::{Error, db::Sqlite},
    form,
};
use santi_model::catalog::{Definition, Kind, Record};
use serde_json::{Map, Value};

use super::writer;

fn decode(kind: Kind, row: &Row) -> Result<Record, Error> {
    let mut values: Map<String, Value> = row
        .cells()
        .iter()
        .map(|(name, cell)| {
            let v = match cell {
                Cell::Int(i) => Value::from(*i),
                Cell::Text(s) => Value::from(s.clone()),
                Cell::Bool(b) => Value::from(*b),
            };
            (name.to_owned(), v)
        })
        .collect();
    values.insert(
        "kind".into(),
        Value::String(kind.unit().to_ascii_lowercase()),
    );
    let definition = serde_json::from_value(Value::Object(values))
        .map_err(|_| Error::Adapt(format!("invalid {} record {}", kind.unit(), row.key())))?;
    Ok(Record {
        id: row.key(),
        definition,
    })
}

pub(in crate::store) async fn snapshot(tx: &mut Tx<'_, Sqlite>) -> Result<Vec<Record>, Error> {
    let mut all = Vec::new();
    for kind in Kind::ALL {
        let result = tx
            .ask(&form(kind.unit()).order("id", keel::Rank::Asc))
            .await?;
        for row in result.rows() {
            all.push(decode(kind, row)?);
        }
    }
    Ok(all)
}

impl Store {
    pub async fn source(&self, kind: Kind, id: i64) -> Result<Option<Record>, String> {
        let row = read::one(&self.core, kind.unit(), "id", &id.to_string()).await?;
        row.as_ref()
            .map(|row| decode(kind, row))
            .transpose()
            .map_err(read::error)
    }

    pub async fn sources(&self) -> Result<Vec<Record>, String> {
        self.core
            .batch(async |tx| snapshot(tx).await)
            .await
            .map_err(read::error)
    }

    pub async fn register(&self, definition: Definition) -> Result<Record, String> {
        self.core
            .batch(async |tx| writer::write(tx, None, definition, None).await)
            .await
            .map_err(read::error)
    }

    pub async fn amend(&self, expected: Record, definition: Definition) -> Result<Record, String> {
        self.amending(expected, definition, None).await
    }

    pub async fn amending(
        &self,
        expected: Record,
        definition: Definition,
        service: Option<&str>,
    ) -> Result<Record, String> {
        self.core
            .batch(async |tx| writer::write(tx, Some(expected), definition, service).await)
            .await
            .map_err(read::error)
    }

    pub async fn withdraw(&self, expected: Record) -> Result<(), String> {
        self.withdrawing(expected, None).await
    }

    pub async fn withdrawing(&self, expected: Record, service: Option<&str>) -> Result<(), String> {
        self.core
            .batch(async |tx| writer::delete(tx, expected, service).await)
            .await
            .map_err(read::error)
    }
}
