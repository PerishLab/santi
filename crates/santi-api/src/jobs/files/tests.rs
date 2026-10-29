use super::directory;
use std::fs::OpenOptions;
use std::os::windows::io::AsRawHandle;
use windows_sys::Win32::Storage::FileSystem::{
    LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY, LockFileEx,
};

#[test]
fn lifetime() {
    let root = tempfile::tempdir().unwrap();
    let directory = directory(root.path());
    directory.prepare().unwrap();
    assert!(!directory.active().unwrap());
    let held = directory.lease().unwrap();
    assert!(directory.active().unwrap());
    assert!(
        directory
            .lease()
            .unwrap_err()
            .contains("job sidecar lease is held")
    );
    let peer = OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.path().join("sidecar.lock"))
        .unwrap();
    let mut request = unsafe { std::mem::zeroed() };
    let result = unsafe {
        LockFileEx(
            peer.as_raw_handle() as _,
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            0,
            1,
            0,
            &mut request,
        )
    };
    assert_eq!(result, 0);
    let error = std::io::Error::last_os_error();
    assert_eq!(error.raw_os_error(), Some(33));
    assert!(super::contended(&error));
    assert!(!super::contended(&std::io::Error::from_raw_os_error(5)));
    assert_eq!(probe(root.path()), 75);
    drop(held);
    assert!(!directory.active().unwrap());
    assert_eq!(probe(root.path()), 0);
    drop(directory.lease().unwrap());
    assert!(!directory.active().unwrap());
    assert!(root.path().join("sidecar.lock").is_file());
}

#[test]
fn refused() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join("sidecar.lock")).unwrap();
    assert!(directory(root.path()).active().is_err());
}

fn probe(root: &std::path::Path) -> i32 {
    std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "jobs::files::tests::worker"])
        .env("SANTI_LOCK_PROBE", root)
        .status()
        .unwrap()
        .code()
        .unwrap()
}

#[test]
fn worker() {
    let Some(root) = std::env::var_os("SANTI_LOCK_PROBE") else {
        return;
    };
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(std::path::Path::new(&root).join("sidecar.lock"))
        .unwrap();
    match file.try_lock() {
        Ok(()) => std::process::exit(0),
        Err(std::fs::TryLockError::WouldBlock) => std::process::exit(75),
        Err(error) => panic!("independent Windows probe failed: {error}"),
    }
}
