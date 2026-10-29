#![cfg(target_os = "linux")]

use santi_api::adaptor::cron::Cron;
use santi_core::service::alarms::Endpoint;
use std::fs::OpenOptions;

#[test]
fn cron() {
    let root = tempfile::tempdir().unwrap();
    let command = root.path().join("crontab");
    plumb::testing::executable(
        &command,
        "#!/bin/sh\nif [ \"$1\" = '-l' ]; then echo 'no crontab for test' >&2; exit 1; fi\nexit 1\n",
    );
    let endpoint = Cron::new(
        command,
        Vec::new(),
        "http://127.0.0.1:43307".into(),
        root.path().to_path_buf(),
    );
    let path = root.path().join("alarm.lock");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .unwrap();
    file.lock().unwrap();
    assert!(
        endpoint
            .apply(&[])
            .unwrap_err()
            .contains("alarm cron lock unavailable")
    );
    file.unlock().unwrap();
    assert!(endpoint.apply(&[]).unwrap().applied);
    file.try_lock().unwrap();
    file.unlock().unwrap();
    let alarm = santi_core::alarm::Alarm {
        id: "probe".into(),
        name: "probe".into(),
        soul: "probe".into(),
        strand: "probe".into(),
        text: "probe".into(),
        cron: "* * * * *".into(),
        generation: 1,
    };
    assert!(
        endpoint
            .apply(&[alarm])
            .unwrap_err()
            .contains("cannot find an executable")
    );
    file.try_lock().unwrap();
    file.unlock().unwrap();
    assert!(path.is_file());
}
