use std::io::Write as _;

use api::config::{Config, ConfigPartial};
use plumb::config::Cascade as _;

const EXAMPLE: &str = r#"
provider = "openai"

[jobs]
acknowledged_retention_seconds = 604800

# One strand-lifetime execution envelope. Provider rounds are per turn; calls
# and captured tool output accumulate until the strand yields to a new one.
[execution]
profile = "runtime_v1"
rounds = 16
calls = 256
output = 4194304
# Each shell call keeps at most this many bytes or lines; a cut result keeps
# its head and tail and saves the complete output under strand://outputs/.
shell = 4096
lines = 256
# Optional experiment: after this many ordinary shell calls, the next provider
# round offers only the feedback tool. Without a caller command, the Soul may
# choose a native gate or product slice.
# feedback_after_calls = 6
# Optional caller-owned mode: feedback becomes a zero-argument tool that runs
# this exact command and workspace. Captured red and green results both reopen
# the ordinary action window.
# feedback_command = "cargo test --locked --workspace"
# feedback_cwd = "strand://product"

# Optional Ed25519 authority for short-lived, per-effect runtime capabilities.
# Supply private_key through SANTI_CAPABILITY_PRIVATE_KEY in production.
[capability]
issuer = ""
audience = ""
key_id = ""
private_key = ""
ttl_seconds = 120

# Lowest explicit layer for every turn shell. Values may be literals or env://
# references. Soul and strand environment resources override this map.
[environment]

[providers.openai]
kind = "openai_responses"
api_key = ""
model = ""
base_url = "https://api.openai.com/v1"
reasoning_effort = ""
summary = ""
bytes = 500000

[providers.deepseek]
kind = "chat_completions"
api_key = ""
model = "deepseek-v4-pro"
base_url = "https://api.deepseek.com"
thinking = ""
reasoning_effort = ""
bytes = 500000

[providers.siliconflow]
kind = "chat_completions"
api_key = ""
model = "zai-org/GLM-5.2"
base_url = "https://api.siliconflow.cn/v1"
thinking = ""
reasoning_effort = ""
bytes = 500000
"#;

fn read(text: &str) -> Config {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(text.as_bytes()).unwrap();
    Config::default().merge(plumb::config::load::<ConfigPartial>(file.path()).unwrap())
}

#[test]
fn example() {
    let held = read(EXAMPLE);
    assert_eq!(held.providers.len(), 3);
    assert_eq!(held.server.grace, 30);
    assert_eq!(held.execution.budget().unwrap().rounds, 16);
    assert_eq!(held.execution.budget().unwrap().shell, 4096);
    assert_eq!(held.execution.budget().unwrap().lines, 256);
    assert_eq!(read("").execution.budget().unwrap().lines, 256);
    assert_eq!(
        held.jobs.retention().unwrap(),
        std::time::Duration::from_secs(604800)
    );
}

#[test]
fn identity() {
    let held = read(
        "[souls.soul_default]\nname = 'Adam Santi'\n[souls.soul_default.channels]\ngithub = 'adam-example'\n",
    );
    let identity = &held.souls["soul_default"];
    assert_eq!(identity.name, "Adam Santi");
    assert_eq!(identity.channels["github"], "adam-example");
    assert!(Config::default().souls.is_empty());
}

#[test]
fn execution() {
    let held = read(
        r#"
[execution]
profile = "operator_v1"
rounds = 8
calls = 64
output = 1048576
shell = 32768
lines = 64
feedback_after_calls = 6
feedback_command = "cargo test --locked --workspace"
feedback_cwd = "strand://product"
"#,
    );
    let budget = held.execution.budget().unwrap();
    assert_eq!(budget.profile, "operator_v1");
    assert_eq!(budget.rounds, 8);
    assert_eq!(budget.calls, 64);
    assert_eq!(budget.output, 1048576);
    assert_eq!(budget.shell, 32768);
    assert_eq!(budget.lines, 64);
    assert_eq!(budget.feedback_after_calls, Some(6));
    assert_eq!(
        budget.feedback_command.as_deref(),
        Some("cargo test --locked --workspace")
    );
    assert_eq!(budget.feedback_cwd.as_deref(), Some("strand://product"));
}

#[test]
fn threshold() {
    let held = read(
        r#"
[execution]
feedback_after_calls = 0
"#,
    );
    assert!(held.execution.budget().is_err());
}

#[test]
fn authority() {
    let orphaned = read(
        r#"
[execution]
feedback_command = "cargo test --workspace"
"#,
    );
    assert!(orphaned.execution.budget().is_err());

    let unowned = read(
        r#"
[execution]
feedback_after_calls = 2
feedback_cwd = "strand://product"
"#,
    );
    assert!(unowned.execution.budget().is_err());
}

#[test]
fn grace() {
    let held = read(
        r#"
[server]
grace = 0
"#,
    );
    assert_eq!(held.server.grace, 0);
}

#[test]
fn listen() {
    let held = read(
        r#"
[listen]
host = "0.0.0.0"
port = 43308
prefix = "/santi"
"#,
    );
    assert_eq!(held.listen.address(), "0.0.0.0:43308");
    assert_eq!(held.listen.prefix, "/santi");
}

#[test]
fn environment() {
    let held = read(
        r#"
[environment]
GLOBAL_LITERAL = "value"
GLOBAL_REFERENCE = "env://HOST_VALUE"
"#,
    );
    assert_eq!(
        held.environment.get("GLOBAL_LITERAL").map(String::as_str),
        Some("value")
    );
    assert_eq!(
        held.environment.get("GLOBAL_REFERENCE").map(String::as_str),
        Some("env://HOST_VALUE")
    );
}

#[test]
fn capability() {
    let held = read(
        r#"
[capability]
issuer = "santi.example"
audience = "stim.reply"
key_id = "key-2026"
private_key = "private-material"
ttl_seconds = 120
"#,
    );
    assert_eq!(held.capability.issuer, "santi.example");
    assert_eq!(held.capability.audience, "stim.reply");
    assert_eq!(held.capability.key_id, "key-2026");
    let shown = format!("{:?}", held.capability);
    assert!(shown.contains("[redacted]"));
    assert!(!shown.contains("private-material"));
}

#[test]
fn legacy() {
    let held = read(
        r#"
provider = "old"

[providers.old]
kind = "openai_responses"
api_key = "key"
model = "model"
reasoning_summary = "auto"
input_budget_bytes = 120000
"#,
    );
    let profile = held.providers.get("old").unwrap().resolve("old").unwrap();
    assert_eq!(profile.bytes(), 120000);
}

#[test]
fn unknown() {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(
        br#"
[providers.bad]
kind = "chat_completions"
api_key = "key"
model = "model"
base_url = "http://127.0.0.1:1"
bytez = 120000
"#,
    )
    .unwrap();
    assert!(plumb::config::load::<ConfigPartial>(file.path()).is_err());
}

#[test]
fn retention() {
    let held = read(
        r#"
[jobs]
acknowledged_retention_seconds = 0
"#,
    );
    assert!(held.jobs.retention().is_err());
}

#[test]
fn memory() {
    assert_eq!(read("").memory.capacity().unwrap(), 32_768);
    assert_eq!(
        read("[memory]\nbytes = 65536\n").memory.capacity().unwrap(),
        65_536
    );
    assert!(read("[memory]\nbytes = 0\n").memory.capacity().is_err());
}
