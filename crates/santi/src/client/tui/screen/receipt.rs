use super::{Beat, Sink, State, Step, Target, UnboundedSender};
use crate::client::send::{Proof, Request as Send, emission, prove};
use std::{future::Future, pin::Pin};

type Check<F> = Option<Pin<Box<F>>>;

#[cfg(test)]
#[path = "receipts.rs"]
mod tests;

pub(super) fn pending<'a>(
    target: Target<'a>,
    receipt: Option<String>,
) -> Check<impl Future<Output = (String, Option<Proof>)> + 'a> {
    receipt.map(|id| Box::pin(inspect(target, id)))
}

pub(super) async fn inspect(target: Target<'_>, receipt: String) -> (String, Option<Proof>) {
    let proof = prove(target, &receipt).await.ok();
    (receipt, proof)
}

pub(super) fn apply(state: &mut State, result: (String, Option<Proof>)) -> Step {
    let (receipt, proof) = result;
    if !state.unsettled || state.receipt.as_deref() != Some(&receipt) {
        return Step::Stay;
    }
    match proof {
        Some(Proof::Completed) => state.heard(Some(Beat::Settled(receipt))),
        Some(Proof::Failed) => state.heard(Some(Beat::Broken(format!(
            "accepted receipt {receipt} is durably failed; do not resend it; inspect with santi receipt {receipt}"
        )))),
        Some(Proof::Pending(_)) | None => Step::Stay,
    }
}

pub(super) async fn dispatch(target: Target<'_>, text: String, sender: UnboundedSender<Beat>) {
    let mut sink = Sink(sender.clone());
    let body = serde_json::json!({ "content": [{ "type": "text", "text": text }] });
    let outcome = emission(
        Send {
            target,
            body,
            watch: true,
        },
        &mut sink,
    )
    .await;
    let beat = match outcome {
        Ok(Some(done)) => Beat::Settled(done.0),
        Ok(None) => Beat::Unsettled("send returned without a receipt proof".to_string()),
        Err(error)
            if error
                .downcast_ref::<crate::client::send::Unsettled>()
                .is_some() =>
        {
            if let Some(receipt) = error
                .downcast_ref::<crate::client::send::Unsettled>()
                .and_then(|unknown| unknown.receipt.clone())
            {
                let _ = sender.send(Beat::Accepted(receipt));
            }
            Beat::Unsettled(format!("{error:#}"))
        }
        Err(error) => Beat::Broken(format!("{error:#}")),
    };
    let _ = sender.send(beat);
}
