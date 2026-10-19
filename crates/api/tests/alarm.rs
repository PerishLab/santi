#![cfg(target_os = "linux")]

use santi_api::adaptor::cron::Cron;
use santi_core::{alarm, service::alarms::Endpoint};

fn endpoint(temp: &tempfile::TempDir) -> Cron {
    let command = temp.path().join("crontab");
    plumb::testing::executable(
        &command,
        r#"#!/bin/sh
if [ "$1" = '-l' ]; then
    if [ -f "$0.table" ]; then cat "$0.table"; exit 0; fi
    echo 'no crontab for test' >&2
    exit 1
fi
if [ -f "$0.refuse" ]; then exit 1; fi
cat > "$0.table"
"#,
    );
    let runner = temp.path().join("santi space/santi");
    std::fs::create_dir_all(runner.parent().unwrap()).unwrap();
    plumb::testing::executable(&runner, "#!/bin/sh\nexit 0\n");
    Cron::new(
        command,
        vec![runner],
        "http://127.0.0.1:43307".into(),
        temp.path().to_path_buf(),
    )
}

fn alarm() -> alarm::Alarm {
    alarm::Alarm {
        id: "alarm_test".into(),
        name: "follow".into(),
        soul: "adam".into(),
        strand: "brain".into(),
        text: "秘密正文 % \n$(false)".into(),
        cron: "*/15 * * * *".into(),
        generation: 1,
    }
}

#[test]
fn lifecycle() {
    let temp = tempfile::tempdir().unwrap();
    let endpoint = endpoint(&temp);
    let table = temp.path().join("crontab.table");
    let unrelated = "# preserve exactly\n5 * * * * printf unrelated\n";
    std::fs::write(&table, unrelated).unwrap();
    let input = alarm();
    assert!(
        endpoint
            .apply(std::slice::from_ref(&input))
            .unwrap()
            .applied
    );
    let first = std::fs::read_to_string(&table).unwrap();
    assert!(first.starts_with(unrelated));
    assert!(first.contains("santi space/santi' --base-url 'http://127.0.0.1:43307' alarm fire 'alarm_test' --generation 1"));
    assert!(!first.contains(&input.text));
    endpoint.apply(std::slice::from_ref(&input)).unwrap();
    assert_eq!(std::fs::read_to_string(&table).unwrap(), first);
    let mut changed = input;
    changed.generation = 2;
    changed.cron = "0 * * * *".into();
    endpoint.apply(&[changed]).unwrap();
    assert!(
        !std::fs::read_to_string(&table)
            .unwrap()
            .contains("--generation 1")
    );
    let restarted = Cron::new(
        temp.path().join("crontab"),
        Vec::new(),
        "http://127.0.0.1:43307".into(),
        temp.path().to_path_buf(),
    );
    restarted.apply(&[]).unwrap();
    assert_eq!(std::fs::read_to_string(&table).unwrap(), unrelated);
}

#[test]
fn retry() {
    let temp = tempfile::tempdir().unwrap();
    let endpoint = endpoint(&temp);
    let refusal = temp.path().join("crontab.refuse");
    std::fs::write(&refusal, "refuse").unwrap();
    let input = alarm();
    assert!(endpoint.apply(std::slice::from_ref(&input)).is_err());
    assert!(!temp.path().join("crontab.table").exists());
    std::fs::remove_file(refusal).unwrap();
    endpoint.apply(std::slice::from_ref(&input)).unwrap();
    let table = temp.path().join("crontab.table");
    let first = std::fs::read_to_string(&table).unwrap();
    std::fs::write(&table, "# unrelated drift\n").unwrap();
    endpoint.apply(std::slice::from_ref(&input)).unwrap();
    let repaired = std::fs::read_to_string(&table).unwrap();
    assert!(repaired.starts_with("# unrelated drift\n"));
    assert_eq!(repaired.matches("alarm fire").count(), 1);
    assert_ne!(first, repaired);
}

#[test]
fn grammar() {
    let temp = tempfile::tempdir().unwrap();
    let endpoint = endpoint(&temp);
    let mut input = santi_core::alarm::Draft {
        soul: "adam".into(),
        strand: "brain".into(),
        name: "follow".into(),
        text: "check".into(),
        cron: "*/15 * * * *".into(),
        generation: None,
    };
    assert!(endpoint.validate(&input).is_ok());
    for expression in [
        "* * * * *\n* * * * * arbitrary",
        "* * * * * command",
        "@reboot",
        "60 * * * *",
        "*/0 * * * *",
        "* * * * +1",
        "* * * * 5-1",
    ] {
        input.cron = expression.into();
        assert!(endpoint.validate(&input).is_err(), "accepted {expression}");
    }
}

#[test]
fn discovery() {
    let temp = tempfile::tempdir().unwrap();
    let endpoint = endpoint(&temp);
    let runner = temp.path().join("santi space/santi");
    std::fs::remove_file(&runner).unwrap();
    let input = alarm();
    assert!(endpoint.apply(std::slice::from_ref(&input)).is_err());
    plumb::testing::executable(&runner, "#!/bin/sh\nexit 0\n");
    assert!(
        endpoint
            .apply(std::slice::from_ref(&input))
            .unwrap()
            .applied
    );
    assert_eq!(
        std::fs::read_to_string(temp.path().join("crontab.table"))
            .unwrap()
            .matches("alarm fire")
            .count(),
        1
    );
}
