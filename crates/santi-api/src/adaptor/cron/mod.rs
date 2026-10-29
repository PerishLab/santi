use santi_core::{alarm, service::alarms::Endpoint};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

mod expression;
mod process;
mod table;

pub struct Cron {
    command: PathBuf,
    runners: Vec<PathBuf>,
    base: String,
    scope: String,
    lock: PathBuf,
}

impl Cron {
    pub fn new(command: PathBuf, runners: Vec<PathBuf>, base: String, runtime: PathBuf) -> Self {
        let scope = hex::encode(Sha256::digest(
            runtime.as_os_str().to_string_lossy().as_bytes(),
        ));
        Self {
            command,
            runners,
            base,
            scope,
            lock: runtime.join("alarm.lock"),
        }
    }

    pub(crate) fn current() -> Self {
        let held = crate::runtime::held();
        let base = match held.bind.parse::<std::net::SocketAddr>() {
            Ok(mut address) => {
                if address.ip().is_unspecified() {
                    address.set_ip(loopback(address.ip()));
                }
                format!("http://{address}")
            }
            Err(_) => String::new(),
        };
        Self::new(
            PathBuf::from("/usr/bin/crontab"),
            crate::jobs::readiness::clients(&held.environment),
            base,
            held.paths.runtime.clone(),
        )
    }
}

impl Endpoint for Cron {
    fn validate(&self, draft: &alarm::Draft) -> Result<(), String> {
        expression::validate(&draft.cron)
    }

    fn apply(&self, alarms: &[alarm::Alarm]) -> Result<alarm::Projection, String> {
        #[cfg(not(target_os = "linux"))]
        {
            let _ = alarms;
            return Err("alarm cron endpoint requires Linux".into());
        }
        #[cfg(target_os = "linux")]
        {
            let lock = std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(&self.lock)
                .map_err(|error| error.to_string())?;
            radicula_lock::attempt(&lock, radicula_lock::Mode::Exclusive)
                .map_err(|error| format!("alarm cron lock unavailable: {error}"))?;
            let outcome = self.converge(alarms);
            let unlocked = radicula_lock::unlock(&lock).map_err(|error| error.to_string());
            outcome?;
            unlocked?;
            Ok(alarm::Projection {
                endpoint: "cron".into(),
                timezone: table::timezone(),
                applied: true,
                observed: None,
                error: None,
            })
        }
    }
}

impl Cron {
    fn converge(&self, alarms: &[alarm::Alarm]) -> Result<(), String> {
        let old = process::read(&self.command)?;
        let next = self.render(&old, alarms)?;
        if old != next {
            if process::read(&self.command)? != old {
                return Err("alarm cron changed during observation".into());
            }
            process::install(&self.command, &next)?;
        }
        if process::read(&self.command)? != next {
            return Err(
                "alarm cron installation outcome is unknown; adaptor will reconcile".into(),
            );
        }
        Ok(())
    }

    fn render(&self, old: &[u8], alarms: &[alarm::Alarm]) -> Result<Vec<u8>, String> {
        let begin = format!("# santi {} begin", self.scope);
        let end = format!("# santi {} end", self.scope);
        let mut block = begin.clone() + "\n";
        if !alarms.is_empty() {
            let runner = self
                .runners
                .iter()
                .find(|path| crate::jobs::readiness::runnable(path))
                .ok_or(
                    "alarm cron cannot find an executable santi client in the declared environment",
                )?;
            for alarm in alarms {
                expression::validate(&alarm.cron)?;
                block.push_str(&format!(
                    "{} {} --base-url {} alarm fire {} --generation {} >/dev/null\n",
                    alarm.cron,
                    table::quote(&runner.display().to_string())?,
                    table::quote(&self.base)?,
                    table::quote(&alarm.id)?,
                    alarm.generation
                ));
            }
        }
        block.push_str(&(end.clone() + "\n"));
        table::replace(
            old,
            &begin,
            &end,
            if alarms.is_empty() { "" } else { &block },
        )
    }
}

fn loopback(address: std::net::IpAddr) -> std::net::IpAddr {
    match address {
        std::net::IpAddr::V4(ip) if ip.is_unspecified() => {
            std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
        }
        std::net::IpAddr::V6(ip) if ip.is_unspecified() => {
            std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)
        }
        other => other,
    }
}
