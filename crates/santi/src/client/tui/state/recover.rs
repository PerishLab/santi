use anyhow::Result;

use super::State;

pub(crate) fn recover<T, Init, Restore>(
    state: &mut State,
    error: anyhow::Error,
    init: Init,
    restore: Restore,
) -> Result<T>
where
    Init: FnOnce() -> std::io::Result<T>,
    Restore: FnOnce(),
{
    state.push(format!(
        "reload failed; still running the old TUI: {error:#}"
    ));
    init().map_err(|init_error| {
        restore();
        anyhow::Error::new(init_error).context("restore old TUI after reload failure")
    })
}
