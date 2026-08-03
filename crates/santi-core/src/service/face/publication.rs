use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::Path;

const MEMORY: &str = "MEMORY.md";
const STAGED: &str = ".MEMORY.md.staged";

pub(super) fn publish(runtime: &Path, soul: &str, bytes: &[u8]) -> Result<(), String> {
    let souls = runtime.join("souls");
    fs::create_dir_all(&souls).map_err(|error| error.to_string())?;
    let seat = souls.join(soul);
    fs::create_dir(&seat).map_err(|error| error.to_string())?;
    let result = home(&seat, bytes).and_then(|()| sync(&souls));
    if result.is_err() {
        fs::remove_dir_all(&seat).ok();
    }
    result
}

fn home(home: &Path, bytes: &[u8]) -> Result<(), String> {
    let memory = home.join("memory");
    fs::create_dir(&memory).map_err(|error| error.to_string())?;
    memoir(&memory, bytes)?;
    sync(home)
}

pub(super) fn memoir(directory: &Path, bytes: &[u8]) -> Result<(), String> {
    let staged = directory.join(STAGED);
    let result = stage(&staged, bytes)
        .and_then(|()| fs::hard_link(&staged, directory.join(MEMORY)).map_err(|e| e.to_string()))
        .and_then(|()| fs::remove_file(&staged).map_err(|e| e.to_string()))
        .and_then(|()| sync(directory));
    if result.is_err() {
        fs::remove_file(staged).ok();
    }
    result
}

fn stage(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    file.write_all(bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())
}

fn sync(directory: &Path) -> Result<(), String> {
    File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(|error| error.to_string())
}
