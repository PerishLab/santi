use std::collections::HashSet;

use santi_error::{Descriptor, Ruled, catalog};
use santi_model::{budget, drive, soul, turn};

fn budgets() -> Vec<Descriptor> {
    use budget::Error::{Context, Execution, Inbox};
    [Context, Execution, Inbox]
        .iter()
        .map(|held| match held {
            Context | Execution | Inbox => held.descriptor(),
        })
        .collect()
}

fn turns() -> Vec<Descriptor> {
    use turn::Error::{Interrupted, Provider, Runtime};
    [Provider, Runtime, Interrupted]
        .iter()
        .map(|held| match held {
            Provider | Runtime | Interrupted => held.descriptor(),
        })
        .collect()
}

fn edges() -> Vec<Descriptor> {
    let drives = [drive::Error::Failed].map(|held| match held {
        drive::Error::Failed => held.descriptor(),
    });
    let souls = [soul::Error::Intervention].map(|held| match held {
        soul::Error::Intervention => held.descriptor(),
    });
    drives.into_iter().chain(souls).collect()
}

fn descriptors() -> Vec<Descriptor> {
    let mut held = vec![
        catalog::UNSAVED,
        catalog::INVALID_ARGUMENT,
        catalog::NOT_FOUND,
        catalog::UNAUTHORIZED,
        catalog::UNAVAILABLE,
        catalog::INTERNAL,
    ];
    held.extend(budgets());
    held.extend(turns());
    held.extend(edges());
    held
}

#[test]
fn lawful() {
    let mut seen = HashSet::new();
    for descriptor in descriptors() {
        let code = descriptor.code;
        assert!(
            code.chars()
                .all(|held| held.is_ascii_lowercase() || held == '.' || held == '_'),
            "code {code} must be lowercase dotted words"
        );
        assert!(seen.insert(code), "code {code} declared twice");
    }
    assert!(
        seen.len() >= 12,
        "expected the full catalog, found {}",
        seen.len()
    );
}
