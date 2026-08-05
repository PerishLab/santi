use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Mutex,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crate::service::Service;

mod notice;

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
    let leases = service.store.scheduled_wakes().await?;
    let leased = leases
        .iter()
        .map(|lease| lease.soul.clone())
        .collect::<BTreeSet<_>>();
    let mut living = living(service).await?;
    living.retain(|soul| !leased.contains(soul));
    let (revision, due) = service.clock.ring(&living)?;
    for soul in due {
        if let Err(error) = notice::regular(service, &soul, revision).await {
            eprintln!("santi: soul clock offer failed soul={soul} detail={error}");
        }
    }
    let now = i64::try_from(epoch()?).map_err(|_| "soul clock time is out of range".to_string())?;
    for lease in leases
        .into_iter()
        .filter(|lease| lease.next_millis.is_some_and(|next| next <= now))
    {
        if let Err(error) = notice::leased(service, &lease, now).await {
            eprintln!(
                "santi: wake lease offer failed soul={} detail={error}",
                lease.soul
            );
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

fn epoch() -> Result<u128, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis())
        .map_err(|error| error.to_string())
}
