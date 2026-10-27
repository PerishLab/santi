use keel::{Core, Op, adapt::db::Sqlite, form};
use santi_estate::{CallDraft, ReplyDraft, Store, StrandDraft, TurnDraft};
use santi_model::turn;
use serde_json::json;
use sqlx::{
    AssertSqlSafe, Connection, Executor,
    sqlite::{SqliteConnectOptions, SqliteConnection},
};

pub const NOW: &str = "2026-10-27T00:00:00.000Z";
const SUDO: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

pub struct Seat {
    pub temp: tempfile::TempDir,
    pub store: Store,
    pub core: Core<Sqlite>,
}

impl Seat {
    pub async fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("estate.sqlite");
        let store = Store::bootstrap(&path, SUDO).await.unwrap();
        store.seed("soul_test", NOW).await.unwrap();
        store
            .create_strand(StrandDraft {
                tag: "strand_test",
                soul: "soul_test",
                label: None,
                parent: None,
                fork: None,
                created: NOW,
            })
            .await
            .unwrap();
        store
            .create_turn(TurnDraft {
                tag: "turn_test",
                strand: "strand_test",
                trigger: turn::Trigger::StrandSend,
                from: 0,
                created: NOW,
            })
            .await
            .unwrap();
        let core = keel::bind(santi_estate::graph(), Sqlite::file(path).await.unwrap())
            .await
            .unwrap();
        Self { temp, store, core }
    }

    pub async fn pair(&self, index: usize) {
        let call = format!("call_{:04}", 1000 - index);
        self.store
            .create_call(CallDraft {
                tag: &call,
                turn: "turn_test",
                tool: "shell",
                arguments: &json!({"sequence": index, "text": "工具🙂"}),
                created: NOW,
            })
            .await
            .unwrap();
        self.store
            .create_reply(ReplyDraft {
                tag: &format!("result_{:04}", 1000 - index),
                call: &call,
                output: (index.is_multiple_of(2)).then_some(&json!({"sequence": index})),
                error: (!index.is_multiple_of(2)).then_some("failed 工具🙂"),
                created: NOW,
            })
            .await
            .unwrap();
    }

    pub async fn key(&self, unit: &str, tag: &str) -> i64 {
        self.core
            .one(&form(unit).when("tag", Op::Eq, tag))
            .await
            .unwrap()
            .unwrap()
            .key()
    }

    pub async fn entry(&self, kind: &str, key: i64, sequence: i64) {
        let strand = self.key("Strand", "strand_test").await.to_string();
        let field = if kind == "tool_call" {
            "call"
        } else {
            "result"
        };
        self.core
            .put(
                "StrandEntry",
                &[
                    ("strand", &strand),
                    ("target_type", kind),
                    (field, &key.to_string()),
                    ("sequence", &sequence.to_string()),
                    ("created", NOW),
                ],
            )
            .await
            .unwrap();
    }

    pub async fn damage(&self, sql: &str) {
        let path = self.temp.path().join("estate.sqlite");
        let mut conn = SqliteConnection::connect_with(
            &SqliteConnectOptions::new()
                .filename(path)
                .foreign_keys(false),
        )
        .await
        .unwrap();
        conn.execute(sqlx::raw_sql(AssertSqlSafe(sql.to_string())))
            .await
            .unwrap();
    }
}
