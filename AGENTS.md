# Agent guide

Read the canonical [PerishLab delivery governance](https://github.com/PerishLab/.github/blob/main/GOVERNANCE.md)
at work start and again before delivery or Issue closure.

Santi keeps individuals (souls) with multiple brains (strands). Inboxes carry
incoming information; tools carry outward actions; compact condenses memory
and sustains thought. External channels are communication windows; their
strand mappings express adaptable working preferences. Brain continuity
spans turns and external work items.

Explicit alarms arrange future inputs to an existing brain. Keel owns their
declarations; adaptors own idempotent platform convergence and recovery.
Inbox acceptance settles delivery independently of later consumption.

Keep changes centered on souls, strands, execution, and their durable evidence.
Product roles and workflows belong to consumers of the runtime.

Santi supplies authoritative runtime facts on demand. System prompts express
stable structural understanding; soul memory preserves individual identity,
judgment and learned experience. Prefer references and runtime retrieval for
information with an existing authority.

## Working constraints

Treat the Keel estate and its sudo possession as one recovery unit when backing
up, restoring, or moving a runtime.

Every estate ownership and reference is a Keel relation, and each resource has
at most one root. A field never holds another row's tag for a reader to
resolve; `crates/santi-estate/tests/model.rs` classifies every text field.
Ending a strand through Keel removes what it owns, and its bytes on disk follow
the committed estate; the start sweep removes bytes no live row owns.
Issue [#226](https://github.com/PerishLab/santi/issues/226) records the design.

Live credentials belong in the ignored `santi.toml`; use `santi.example.toml`
for tracked configuration changes.

The runtime selects history independently of provider reasoning continuity.
Thinking summaries are inspection text; original protocol continuation state is
recorded separately on the provider round. Chat profiles explicitly opt into
`replay = "history"` when their protocol requires it. The adaptor replays only
compatible state in the selected history; compacts do not reload archived
reasoning. Estimates and compact previews follow the same provider projection.
Issue [#237](https://github.com/PerishLab/santi/issues/237) records DeepSeek's
contract and measured consumption. Responses remains stateless as established
by the router probe in Issue [#216](https://github.com/PerishLab/santi/issues/216).

Release work ends at artifact publication. Host deployment is separate
operational work.

## Source authorities

The workspace manifests own crate membership and dependency boundaries.
`crates/santi-core/src/assembly/prompt.rs` owns the built-in soul and strand
constitution; Issue [#187](https://github.com/PerishLab/santi/issues/187) records
the reference method for diagnosing soul behavior through its observable inputs.
`crates/santi-core/src/service/flow/memory.rs` owns memory pressure
and maintenance.

For execution and recovery changes, follow
`crates/santi-core/src/service/flow` into `crates/santi-estate/src/store`.
Receipt, completion, and effect behavior is exercised in
`crates/santi-estate/tests/completion`,
`crates/santi-estate/tests/operations/terminal.rs`, and
`crates/santi-estate/tests/effects.rs`.

The binaries' `--help` and the server's OpenAPI export own the public interface.
`plumb.toml` owns release targets and placement; `packaging/deb` owns Debian
host integration. Consult current Plumb and Wharf help for delivery.

## Verification

```sh
cargo fmt --all
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

Run `plumb doctor .` and `ectropy .` before and after repository shape changes.

For a real-provider turn, reuse the ignored repo-root `santi.toml` when it is
configured. Run `santi-api --config santi.toml serve` in a separate terminal, then:

```sh
SID=$(santi strand create | jq -r .strand.id)
SANTI_STRAND_ID=$SID santi strand send 'Reply with exactly: OK' --watch
```

Inspect an accepted send with `santi receipt <inbox>` and
`santi strand runtime <strand>` when its watch result remains unknown.
Stop the server after the check.
