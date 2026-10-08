# Agent guide

Read the canonical [PerishLab delivery governance](https://github.com/PerishLab/.github/blob/main/GOVERNANCE.md)
at work start and again before delivery or Issue closure.

Santi is a standalone agent runtime. Keep changes centered on souls, strands,
execution, and their durable evidence. Product roles and workflows belong to
consumers of the runtime.

## Working constraints

Treat the Keel estate and its sudo possession as one recovery unit when backing
up, restoring, or moving a runtime.

Live credentials belong in the ignored `santi.toml`; use `santi.example.toml`
for tracked configuration changes.

Release work ends at artifact publication. Host deployment is separate
operational work.

## Source authorities

The workspace manifests own crate membership and dependency boundaries.
`crates/santi-core/src/assembly/prompt.rs` owns the built-in soul and strand
constitution; `crates/santi-core/src/service/flow/memory.rs` owns memory pressure
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
configured. Run `santi-api serve` in a separate terminal, then:

```sh
SID=$(santi strand create | jq -r .strand.id)
SANTI_STRAND_ID=$SID santi strand send 'Reply with exactly: OK' --watch
```

Inspect an accepted send with `santi receipt <inbox>` and
`santi strand runtime <strand>` when its watch result remains unknown.
Stop the server after the check.
