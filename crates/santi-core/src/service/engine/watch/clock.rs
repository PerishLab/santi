use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Mutex,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde_json::json;
use sha2::{Digest, Sha256};

use crate::service::Service;
use crate::{ingest, message, strand};

pub(in crate::service) const CADENCE: Duration = Duration::from_secs(10 * 60);
pub(in crate::service) const WINDOW: Duration = Duration::from_secs(60);
const LABEL: &str = "santi:clock";

pub(in crate::service) struct Clock {
    cadence: Duration,
    window: Duration,
    state: Mutex<State>,
}

struct State {
    last: u128,
    active: BTreeMap<String, Seat>,
}

struct Seat {
    since: Instant,
    opened: bool,
}

impl Clock {
    pub(in crate::service) fn new(cadence: Duration, window: Duration) -> Result<Self, String> {
        if cadence.is_zero() || window.is_zero() {
            return Err(
                "soul clock cadence and observation window must be greater than zero".to_string(),
            );
        }
        let now = epoch()?;
        Ok(Self {
            cadence,
            window,
            state: Mutex::new(State {
                last: now / cadence.as_millis(),
                active: BTreeMap::new(),
            }),
        })
    }

    pub(in crate::service) fn cadence(&self) -> Duration {
        self.cadence
    }

    pub(in crate::service) fn window(&self) -> Duration {
        self.window
    }

    fn ring(&self, living: &BTreeSet<String>) -> Result<(i64, BTreeSet<String>), String> {
        let now = epoch()?;
        let tick = now / self.cadence.as_millis();
        let instant = Instant::now();
        let mut state = self.state.lock().unwrap();
        state.active.retain(|soul, _| living.contains(soul));
        let periodic = tick > state.last;
        if periodic {
            state.last = tick;
        }
        let mut due = BTreeSet::new();
        for soul in living {
            let seat = state.active.entry(soul.clone()).or_insert(Seat {
                since: instant,
                opened: false,
            });
            if !seat.opened && instant.duration_since(seat.since) >= self.window {
                seat.opened = true;
                due.insert(soul.clone());
            }
            if periodic {
                due.insert(soul.clone());
            }
        }
        let revision =
            i64::try_from(now).map_err(|_| "soul clock time is out of range".to_string())?;
        Ok((revision, due))
    }
}

pub(in crate::service) async fn ring(service: &Service) -> Result<(), String> {
    let living = living(service).await?;
    let (revision, due) = service.clock.ring(&living)?;
    for soul in due {
        if let Err(error) = offer(service, &soul, revision).await {
            eprintln!("santi: soul clock offer failed soul={soul} detail={error}");
        }
    }
    Ok(())
}

async fn living(service: &Service) -> Result<BTreeSet<String>, String> {
    let turns = service
        .controls
        .lock()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    let mut souls = BTreeSet::new();
    for turn in turns {
        let Some(turn) = service.store.turn(&turn).await? else {
            continue;
        };
        if let Some(strand) = service.store.strand(&turn.strand).await? {
            souls.insert(strand.soul);
        }
    }
    for record in service.store.active_jobs().await? {
        souls.insert(record.job.origin.soul);
    }
    Ok(souls)
}

async fn offer(service: &Service, soul: &str, revision: i64) -> Result<(), String> {
    let observed = crate::stamped(UNIX_EPOCH + Duration::from_millis(revision as u64))?;
    let strand = service
        .store
        .selected(
            &strand::Selector::ByLabel {
                soul: soul.to_string(),
                label: LABEL.to_string(),
            },
            &observed,
        )
        .await?;
    let content = message::Content::text(format!("current_time: {observed}"));
    let encoded = serde_json::to_vec(&content).map_err(|error| error.to_string())?;
    let digest = format!("{:x}", Sha256::digest(encoded));
    let source = ingest::Source::new("clock")
        .with_ref(soul.to_string())
        .with_metadata(json!({"schema": "santi.soul.clock.v1"}));
    let key = format!("clock/{soul}");
    let inbox = crate::tag("inbox");
    let offered = service
        .store
        .offer_notice(
            santi_estate::NoticeDraft {
                tag: &inbox,
                strand: &strand.id,
                key: &key,
                revision,
                digest: &digest,
                content: &content,
                source: &source,
                causes: &[],
                created: &observed,
            },
            500,
        )
        .await?;
    service.dispatched().await;
    if offered.inserted
        && let Some(inbox) = offered.inbox
    {
        service.inboxes.lock().unwrap().insert(strand.id, inbox);
    }
    Ok(())
}

fn epoch() -> Result<u128, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .map_err(|error| error.to_string())
}
