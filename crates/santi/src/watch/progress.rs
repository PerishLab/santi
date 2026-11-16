use crate::client::Target;
use serde::Deserialize;
use std::time::Duration;

#[derive(Deserialize)]
struct Snapshot {
    epoch: String,
    revision: u64,
    strand: String,
    current: Option<Current>,
}

#[derive(Deserialize)]
struct Current {
    turn: String,
    round: u64,
    phase: Phase,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Phase {
    Preparing,
    Requesting,
    Responding,
    Generating,
    Calling,
    Invoking,
}

impl Phase {
    fn label(&self) -> &'static str {
        match self {
            Self::Preparing => "preparing",
            Self::Requesting => "requesting",
            Self::Responding => "responding",
            Self::Generating => "generating",
            Self::Calling => "calling",
            Self::Invoking => "invoking",
        }
    }
}

pub(super) async fn read(target: Target<'_>, turn: Option<&str>) -> String {
    let Some(turn) = turn else {
        return "activity unknown for this receipt: no current driving turn was established".into();
    };
    let snapshot = tokio::time::timeout(Duration::from_secs(5), fetch(target)).await;
    match snapshot {
        Ok(Some(snapshot)) => describe(snapshot, target.strand, turn),
        _ => "activity unknown for this receipt: snapshot unavailable or invalid".into(),
    }
}

async fn fetch(target: Target<'_>) -> Option<Snapshot> {
    let url = format!("{}/api/v1/strands/{}/activity", target.base, target.strand);
    let response = target.client.get(url).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.json().await.ok()
}

fn describe(snapshot: Snapshot, strand: &str, turn: &str) -> String {
    if snapshot.strand != strand || snapshot.epoch.is_empty() {
        return "activity unknown for this receipt: snapshot identity invalid".into();
    }
    let Some(current) = snapshot.current.filter(|current| current.turn == turn) else {
        return "activity unknown for this receipt: no matching current turn; absence does not prove completion or idleness".into();
    };
    format!(
        "observed activity for turn {turn}: phase={}, round={}, process-local revision={}; this snapshot does not prove ongoing advancement or completion",
        current.phase.label(),
        current.round,
        snapshot.revision
    )
}
