use std::{
    collections::BTreeMap,
    ffi::{CString, OsStr, OsString},
    os::unix::ffi::OsStrExt,
    os::unix::fs::FileTypeExt,
    path::{Path, PathBuf},
};

use santi_core::environment::{Declaration, resolve};

use super::{Check, Shell};

pub(super) fn executable(name: &str, path: &OsStr) -> Option<PathBuf> {
    std::env::split_paths(path)
        .map(|directory| directory.join(name))
        .find(|candidate| runnable(candidate))
        .and_then(|path| std::path::absolute(path).ok())
}

fn runnable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let Ok(path) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    unsafe { libc::access(path.as_ptr(), libc::X_OK) == 0 }
}

pub(super) fn shell(declared: &BTreeMap<String, String>) -> (Shell, Vec<Check>) {
    let mut checks = Vec::new();
    let mut shell = Shell {
        source: "process",
        reference: None,
        entries: 0,
        client: None,
    };
    let path = match declared.get("PATH") {
        Some(value) => configured(value, &mut shell),
        None => std::env::var_os("PATH"),
    };
    if let Some(path) = path.filter(|path| !path.is_empty()) {
        shell.entries = std::env::split_paths(&path).count();
        shell.client = executable("santi", &path);
        checks.push(Check::ready(
            "path",
            "The global shell search path is resolved; values are omitted.",
        ));
    } else {
        checks.push(Check::missing("path", "The global shell PATH is absent, empty or unresolved.", "Set environment.PATH in the selected config or supply its env:// reference to this process."));
    }
    checks.push(if shell.client.is_some() {
        Check::ready("client", "An executable santi client is reachable through the global shell PATH.")
    } else {
        Check::missing("client", "No executable santi client is reachable through the global shell PATH.", "Install the client through its stable manager and include its bin directory in environment.PATH for the runtime user.")
    });
    (shell, checks)
}

fn configured(value: &str, shell: &mut Shell) -> Option<OsString> {
    shell.source = "configuration";
    shell.reference = value
        .strip_prefix("env://")
        .map(str::trim)
        .filter(|name| {
            !name.is_empty()
                && name
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '_')
        })
        .map(str::to_string);
    let resolved = resolve(
        [Declaration {
            scope: "global".to_string(),
            name: "PATH".to_string(),
            value: value.to_string(),
        }],
        &|name| std::env::var(name).ok(),
    );
    if !resolved.unresolved.is_empty() {
        return None;
    }
    resolved.values.get("PATH").map(OsString::from)
}

pub(super) fn session() -> Vec<Check> {
    let runtime = std::env::var_os("XDG_RUNTIME_DIR").filter(|path| !path.is_empty());
    let directory = runtime
        .as_ref()
        .is_some_and(|path| Path::new(path).is_dir());
    let mut checks = vec![if directory {
        Check::ready(
            "runtime",
            "XDG_RUNTIME_DIR selects an existing runtime directory.",
        )
    } else {
        Check::missing(
            "runtime",
            "XDG_RUNTIME_DIR is absent or does not select a directory.",
            "Run this check as the intended service user in its systemd/PAM login environment; ensure the user runtime directory exists.",
        )
    }];
    let address =
        std::env::var_os("DBUS_SESSION_BUS_ADDRESS").is_some_and(|value| !value.is_empty());
    let socket = runtime
        .map(PathBuf::from)
        .map(|path| path.join("bus"))
        .is_some_and(|path| {
            path.metadata()
                .is_ok_and(|metadata| metadata.file_type().is_socket())
        });
    checks.push(if address || socket {
        Check::ready("bus", "A user-bus address or runtime socket is present; the manager query checks reachability.")
    } else {
        Check::missing("bus", "Neither a user-bus address nor the default runtime bus socket is present.", "Ensure the persistent user manager and its bus are available to the runtime user's process environment.")
    });
    checks
}
