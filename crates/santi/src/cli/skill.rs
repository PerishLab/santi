use anyhow::{Context, Result};
use plumb::skill::{
    Kit,
    command::{Command, Deed},
};

const RELEASES: &str = "https://releases.santi.perish.uk";
const DEPOT: &str = "https://depot.santi.perish.uk";

#[locus::trace(with = plumb::observation::view())]
pub(crate) fn run(deed: Deed) -> Result<()> {
    let home = crate::config::home().context("santi skill: no data home; set SANTI_HOME")?;
    let kit = Kit {
        name: "santi".to_string(),
        home: plumb::config::home().context("santi skill: no user home")?,
        state: home.join("state/skills.json"),
        url: plumb::config::value("SANTI_RELEASES").unwrap_or_else(|| RELEASES.to_string()),
    };
    let depot = kit.depot(DEPOT, "santi", plumb::version!("SANTI"));
    anyhow::ensure!(
        Command("santi").run(&depot, deed) == 0,
        "santi skill operation failed"
    );
    Ok(())
}

pub(super) fn focused(command: clap::Command) -> clap::Command {
    command
        .mut_args(|arg| {
            if arg.is_global_set() {
                arg.hide(true)
            } else {
                arg
            }
        })
        .mut_subcommands(|command| command.defer(focused))
}
