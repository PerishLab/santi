use super::tool::{self, Call};
use super::{Store, read};
use keel::{Op, Rank, form};
use santi_model::effect;

mod projection;
mod types;
mod write;

pub use types::{Effect, Redemption};

impl Store {
    pub async fn uncertain(
        &self,
        tag: &str,
        draft: tool::Reply<'_>,
    ) -> Result<santi_model::tool::Reply, String> {
        let output = draft
            .output
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| error.to_string())?;
        self.core
            .batch(async |tx| {
                write::Writer::new(tx)
                    .uncertain(tag, draft, output.as_deref())
                    .await
            })
            .await
            .map_err(read::error)?;
        self.reply(draft.tag)
            .await?
            .ok_or_else(|| "created uncertain tool result missing".to_string())
    }

    pub async fn invoke(
        &self,
        call: Call<'_>,
        effect: Option<Effect<'_>>,
    ) -> Result<(santi_model::tool::Call, Option<effect::Effect>), String> {
        let arguments = serde_json::to_string(call.arguments).map_err(|error| error.to_string())?;
        let metadata = effect
            .as_ref()
            .and_then(|effect| effect.metadata)
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| error.to_string())?;
        let called = call.tag.to_string();
        let staged = effect.as_ref().map(|effect| effect.tag.to_string());
        self.core
            .batch(async |tx| {
                tool::call(tx, call, &arguments).await?;
                if let Some(effect) = effect {
                    write::Writer::new(tx)
                        .prepare(effect, metadata.as_deref())
                        .await?;
                }
                Ok(())
            })
            .await
            .map_err(read::error)?;
        let call = self
            .call(&called)
            .await?
            .ok_or_else(|| "prepared invocation call missing".to_string())?;
        let effect = match staged {
            Some(tag) => Some(
                self.effect(&tag)
                    .await?
                    .map(|status| status.effect)
                    .ok_or_else(|| "prepared invocation effect missing".to_string())?,
            ),
            None => None,
        };
        Ok((call, effect))
    }

    pub async fn stage(&self, draft: Effect<'_>) -> Result<effect::Effect, String> {
        let metadata = draft
            .metadata
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| error.to_string())?;
        self.core
            .batch(async |tx| {
                write::Writer::new(tx)
                    .prepare(draft, metadata.as_deref())
                    .await
            })
            .await
            .map_err(read::error)?;
        self.effect(draft.tag)
            .await?
            .map(|status| status.effect)
            .ok_or_else(|| "prepared effect missing".to_string())
    }

    pub async fn effect(&self, tag: &str) -> Result<Option<effect::Status>, String> {
        projection::status(self, tag).await
    }

    pub async fn dispatching(&self, tag: &str, occurred: &str) -> Result<effect::Effect, String> {
        shift(
            self,
            write::Shift {
                tag,
                expected: &["prepared"],
                state: effect::State::Dispatching,
                error: None,
                occurred,
            },
        )
        .await
    }

    pub async fn suspect(
        &self,
        tag: &str,
        evidence: &str,
        occurred: &str,
    ) -> Result<effect::Effect, String> {
        shift(
            self,
            write::Shift {
                tag,
                expected: &["dispatching"],
                state: effect::State::Unknown,
                error: Some(evidence),
                occurred,
            },
        )
        .await
    }

    pub async fn settle(
        &self,
        tag: &str,
        outcome: effect::Outcome,
        evidence: &str,
        occurred: &str,
    ) -> Result<Option<effect::Status>, String> {
        if evidence.trim().is_empty() {
            return Err("effect resolution evidence must not be empty".to_string());
        }
        if self.effect(tag).await?.is_none() {
            return Ok(None);
        }
        shift(
            self,
            write::Shift {
                tag,
                expected: &["unknown"],
                state: effect::State::Settled(outcome),
                error: None,
                occurred,
            },
        )
        .await?;
        self.effect(tag).await
    }

    pub async fn redeem(
        &self,
        tag: &str,
        draft: Redemption<'_>,
    ) -> Result<santi_model::tool::Reply, String> {
        let output = draft
            .output
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| error.to_string())?;
        let result = draft.result.to_string();
        self.core
            .batch(async |tx| {
                write::Writer::new(tx)
                    .redeem(tag, draft, output.as_deref())
                    .await
            })
            .await
            .map_err(read::error)?;
        self.reply(&result)
            .await?
            .ok_or_else(|| "created effect tool result missing".to_string())
    }

    pub async fn reconcile(&self, turn: &str, occurred: &str) -> Result<(), String> {
        self.core
            .batch(async |tx| reconcile(tx, turn, occurred).await)
            .await
            .map_err(read::error)
    }
}

async fn shift(store: &Store, draft: write::Shift<'_>) -> Result<effect::Effect, String> {
    let tag = draft.tag;
    store
        .core
        .batch(async |tx| write::Writer::new(tx).shift(draft).await.map(|_| ()))
        .await
        .map_err(read::error)?;
    store
        .effect(tag)
        .await?
        .map(|status| status.effect)
        .ok_or_else(|| "shifted effect missing".to_string())
}

pub(in crate::store) async fn reconcile(
    tx: &mut keel::Tx<'_, keel::adapt::db::Sqlite>,
    turn: &str,
    occurred: &str,
) -> Result<(), keel::adapt::Error> {
    let turn = write::Writer::new(tx).relation("Turn", turn).await?;
    let effects = tx
        .ask(
            &form("StrandEffect")
                .when("turn", Op::Eq, &turn.key().to_string())
                .order("created", Rank::Asc)
                .order("tag", Rank::Asc),
        )
        .await?;
    for effect in effects.rows() {
        let tag = effect
            .text("tag")
            .ok_or_else(|| keel::adapt::Error::Adapt("effect tag missing".into()))?;
        apply(tx, tag, effect.text("state"), occurred).await?;
    }
    Ok(())
}

async fn apply(
    tx: &mut keel::Tx<'_, keel::adapt::db::Sqlite>,
    tag: &str,
    state: Option<&str>,
    occurred: &str,
) -> Result<(), keel::adapt::Error> {
    match state {
        Some("prepared") => {
            write::Writer::new(tx)
                .shift(write::Shift {
                    tag,
                    expected: &["prepared"],
                    state: effect::State::Settled(effect::Outcome::Unapplied),
                    error: None,
                    occurred,
                })
                .await?;
        }
        Some("dispatching") => {
            write::Writer::new(tx)
                .shift(write::Shift {
                    tag,
                    expected: &["dispatching"],
                    state: effect::State::Unknown,
                    error: None,
                    occurred,
                })
                .await?;
        }
        _ => {}
    }
    Ok(())
}
