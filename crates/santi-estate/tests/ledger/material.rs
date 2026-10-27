use super::{NOW, SUDO};
use santi_estate::{Bind, Store, StrandDraft, Unbind};
use santi_model::{environ::Scope, material};

mod layers;
mod records;

struct Fixture {
    temp: tempfile::TempDir,
    store: Store,
}

impl Fixture {
    async fn open() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::bootstrap(temp.path().join("estate.sqlite"), SUDO)
            .await
            .unwrap();
        store.seed("soul", NOW).await.unwrap();
        for tag in ["first", "second"] {
            store
                .create_strand(StrandDraft {
                    tag,
                    soul: "soul",
                    label: None,
                    parent: None,
                    fork: None,
                    created: NOW,
                })
                .await
                .unwrap();
        }
        Self { temp, store }
    }

    async fn bind(
        &self,
        scope: Scope,
        operation: &str,
        package: &material::Package,
    ) -> Result<material::Change, String> {
        self.store
            .bind(Bind {
                scope,
                owner: owner(scope),
                package,
                operation,
                created: NOW,
            })
            .await
    }

    async fn drop(&self, scope: Scope, operation: &str) -> material::Change {
        self.store
            .unbind(Unbind {
                scope,
                owner: owner(scope),
                reference: "perish:plumb",
                operation,
                created: NOW,
            })
            .await
            .unwrap()
    }

    async fn selection(&self, tag: &str) -> material::Selection {
        self.store.bindings(Scope::Strand, tag).await.unwrap()
    }
}

fn owner(scope: Scope) -> &'static str {
    if scope == Scope::Soul {
        "soul"
    } else {
        "first"
    }
}

fn package(text: &str) -> material::Package {
    material::Package {
        material: material::Descriptor {
            reference: "perish:plumb".into(),
            name: "plumb".into(),
            description: "Operate Plumb.".into(),
            revision: String::new(),
            entry: vec!["SKILL.md".into()],
            blobs: vec![],
            provenance: material::Provenance {
                source: "perish".into(),
                format: "skill".into(),
                version: Some("1.0".into()),
                keeper: Some("plumb".into()),
            },
        },
        contents: vec![material::Content {
            name: "SKILL.md".into(),
            mime: "text/markdown".into(),
            text: text.into(),
        }],
    }
    .sealed()
    .unwrap()
}
