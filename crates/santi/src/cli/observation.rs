use locus::{Candidate, Config, Context, Engine, Policy, Role};
use serde::Deserialize;
use serde_json::json;
use std::path::PathBuf;
use std::sync::OnceLock;

#[derive(Default, Deserialize)]
struct File {
    #[serde(default)]
    locus: Settings,
}

#[derive(Default, Deserialize)]
struct Settings {
    #[serde(default)]
    enabled: bool,
    endpoint: Option<String>,
}

struct Seat {
    engine: Engine,
    context: Context,
}

static SEAT: OnceLock<Seat> = OnceLock::new();

pub(crate) struct Run {
    context: Context,
}

impl Run {
    pub(crate) fn start(command: &str) -> Option<Self> {
        match load(command) {
            Ok(run) => run,
            Err(error) => {
                eprintln!("santi locus: {error}");
                None
            }
        }
    }

    pub(crate) fn finish(self, code: i32) {
        let Some((engine, _)) = view() else {
            return;
        };
        let candidate = Candidate::event(json!({"event": "cli.finish", "code": code}));
        let _ = engine.append(&self.context, candidate);
    }
}

pub(crate) fn view() -> Option<(&'static Engine, &'static Context)> {
    SEAT.get().map(|seat| (&seat.engine, &seat.context))
}

fn load(command: &str) -> Result<Option<Run>, String> {
    let Some(home) = crate::config::home() else {
        return Ok(None);
    };
    let path = home.join("config.toml");
    let file: File = match plumb::config::load(&path) {
        Ok(file) => file,
        Err(plumb::config::Error::Read { source, .. })
            if source.kind() == std::io::ErrorKind::NotFound =>
        {
            return Ok(None);
        }
        Err(plumb::config::Error::Parse { path, .. }) => {
            return Err(format!(
                "invalid observation configuration in {}",
                path.display()
            ));
        }
        Err(error) => return Err(error.to_string()),
    };
    if !file.locus.enabled {
        return Ok(None);
    }
    let endpoint = file.locus.endpoint.unwrap_or_else(|| {
        plumb::config::value("LOCUS_API").unwrap_or_else(|| "http://127.0.0.1:43308".into())
    });
    build(command, endpoint, home.join("state/locus")).map(Some)
}

fn build(command: &str, endpoint: String, buffer: PathBuf) -> Result<Run, String> {
    let engine = Engine::bootstrap(Config::new(
        Policy::default()
            .producer("santi")
            .reporter(locus::reporter::Spec::api(endpoint, buffer)),
    ))
    .map_err(|error| error.to_string())?;
    let candidate = Candidate::context().ensure(Role::trace()).explicit(
        Role::new("santi.command").map_err(|error| error.to_string())?,
        locus::Key::new(command).map_err(|error| error.to_string())?,
    );
    let context = engine
        .append(&Context::empty(), candidate)
        .map_err(|error| error.to_string())?
        .context();
    let cycle = engine
        .append(
            &context,
            Candidate::event(json!({"event": "cli.start"})).ensure(Role::span()),
        )
        .map_err(|error| error.to_string())?
        .context();
    SEAT.set(Seat { engine, context })
        .map_err(|_| "observation already initialized".to_string())?;
    Ok(Run { context: cycle })
}

pub(crate) fn name(command: &crate::cli::Command) -> &'static str {
    use crate::cli::Command;
    match command {
        Command::Health => "health",
        Command::Tui { .. } => "tui",
        Command::Errors { .. } => "errors",
        Command::Receipt { .. } => "receipt",
        Command::Effect(_) => "effect",
        Command::Strand(_) => "strand",
        Command::Compact(_) => "compact",
        Command::Job(_) => "job",
        Command::Wake(_) => "wake",
        Command::Turn(_) => "turn",
        Command::Environment(_) => "env",
        Command::Webhook(_) => "webhook",
    }
}
