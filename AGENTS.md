# Agent guide

`santi` is a standalone agent runtime. Treat this repo as runtime-first: there
is no product layer here, and none should be added speculatively.

## Layout

```text
crates/
  api/             # the `santi-api` binary: config, bootstrap, serve, local ops
  santi-core/      # runtime model + service (turns, assembly, objects, workspace)
  santi-estate/    # Keel graph + durable ceremonies and projections
  santi-provider/  # ProviderClient boundary; keeps santi-core provider-agnostic
  santi-api/       # HTTP/SSE/OpenAPI server library over santi-core
  santi/           # the `santi` binary: transport-only HTTP client and TUI
packaging/deb/     # the santi-api Debian placement: control, maintainer scripts, root/ payload
```

## Boundaries

- `santi-core` is provider-agnostic. Provider specifics live behind
  `santi-provider::ProviderClient`.
- `santi-api` is the only network boundary. Browser/host-facing shapes are
  owned here, not in `santi-core`.
- `api` ships `santi-api`, the server entry. It owns config resolution,
  bootstrap, serving, OpenAPI export, and local runtime operations.
- `santi` keeps its transport-only HTTP client boundary. It must never depend on
  or call `santi-api` or `santi-core` in process. HTTP stays the only way in.
- Both binaries install the product identity `plumb::identity!("SANTI")` before
  parsing arguments and report `<binary> <marker>` through
  `plumb::version!("SANTI")`.

## Build & checks

```sh
cargo fmt --all
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

Run `plumb doctor .` and `ectropy .` before and after changing repository
shape. The repository carries no CI workflow; guard hooks projected by
`plumb configuration install` prove each staged tree.

## Trigger a single turn locally (hot path)

To exercise a real end-to-end turn, prefer reusing the repo-root
`santi.toml` — it already configures a working provider, so no ad-hoc
config or env wiring is needed. `santi-api serve` reads `./santi.toml`
by default; drive one turn and stop when it lands:

```sh
santi-api serve &                                       # reads ./santi.toml
SID=$(santi strand create | jq -r .strand.id)
SANTI_STRAND_ID=$SID santi strand send 'Reply with exactly: OK' --watch
```

`--watch` subscribes before sending and exits successfully only after the
accepted message's matching receipt is proven to have reached durable
`completed`. It does not prove strand idleness, and queued follow-on work may
remain after success. Durable `failed` is an error. A pending receipt or
unavailable proof remains `state_unknown`: after sixty seconds without an event
or ten minutes total, do not resend the accepted message; inspect it with
`santi receipt <inbox>` and `santi strand runtime <strand>`, then resume the
blocking condition or explicitly redrive it with `santi strand drive <strand>`.
Silence is never success. By default, watch output is filtered human-readable
milestones for interactive use.

For raw/debug automation, pass `--watch-format raw`; it relays event JSON (one
object per line, same payload shape as `strand events`). Distill the reply with
jq:

```sh
… strand send '…' --watch --watch-format raw \
  | jq -rc 'select(.payload.type=="message" and .payload.beat=="completed")
            | .payload.message.content_text'
```

`--strand`/`SANTI_STRAND_ID` set a default strand id; `--soul`/`SANTI_SOUL_ID`
pick a non-default soul (empty → the runtime's default soul; an unknown soul is
rejected, not silently created). To address a soul ad hoc without a default:
`santi --soul <id> strand send <strand_id> '…'`.

## Conventions

- Edition 2024, MIT. Workspace dependencies are pinned in the root
  `Cargo.toml`; crates reference them with `.workspace = true`.
- GitHub `PerishLab/santi` is the canonical repository.
- Santi's runtime boundary stops at its executables. `packaging/deb` is the
  host placement for the server, not runtime architecture; its maintainer
  scripts create the `santi` system user, enable but never start
  `santi.service`, stop it only on removal, and never delete
  `/home/santi/.santi`.
- Runtime secrets live in `santi.toml`; it is gitignored. Never commit live
  credentials; `santi.example.toml` is the tracked runtime template.

## Release

- Plumb owns repository governance, release markers, and landing; wharf builds,
  binds, and distributes each release. `plumb.toml` is the release
  declaration. Read their current help and rules; do not restate a release
  workflow or changelog shape here.
- One marker covers both executables: `santi` ships for Linux x86_64 and macOS
  arm64 through the manager; `santi-api` ships for Linux x86_64 only, is not
  installed by the manager, and is placed as the `santi-api` `.deb`. The deb
  carries only `santi-api`, never the client.
- A placement is published, never deployed. Deploying a host is outside this
  repository.
