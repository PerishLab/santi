# Design

Santi is a standalone agent runtime. It owns souls, strands, turns, context
assembly, provider-neutral execution, local objects, workspace, memory, and
their durable evidence. HTTP is the only runtime entrance.

## Estate and entry

Explicit bootstrap creates the Keel estate and owner-only sudo custody.
Ordinary serve binds an already initialized estate and never mints replacement
custody. The estate and sudo possession form one recovery unit.

The server binary owns config, bootstrap, serving, OpenAPI export, and local
operations. The client is transport-only and never links runtime crates. The
provider boundary remains behind `ProviderClient`; provider vocabulary does not
enter the core runtime.

## Receipts and recovery

Every accepted send returns a durable inbox receipt. Completion means its
assistant turn completed and was persisted. Driver recovery or incident
resolution alone never completes a receipt. Reconstructed migration evidence
is distinguishable from live transitions.

A failed receipt is retried only by an explicit drive or a compact that resolves
its incident. Ordinary boot and completion pokes do not replay it. Recovery
reuses durable confirmed effect results rather than repeating external work.

## Effects

Each shell command creates a durable effect attempt. `prepared` has not begun
dispatch. An interrupted `dispatching` attempt becomes `unknown`: the runtime
cannot prove whether the external action occurred and never repeats it
automatically. A mechanically rejected spawn is `not_dispatched`; a captured
result is `confirmed`.

Only an unknown effect accepts explicit operator resolution as applied or not
applied. Resolution retains evidence but neither retries the command nor changes
its turn or receipt state.

## Detached jobs and attention

A soul-owned Job is durable before its detached sidecar starts. Creation
success proves the specification and initial claimed state, not that the
payload is running or complete. The process receives origin locators but never
inherits the single-use creation capability. Cold start reconciles each active
stamp independently and never replays uncertainty.

Running turns and active jobs receive coalesced wall-clock attention after one
minute and at ten-minute boundaries. The clock exposes absolute time, wake
lease, renewal, and silence only. It never inspects work, diagnoses delay,
recommends action, or inherits execution authority.

## Downstreams

A downstream owns one non-overlapping opaque label zone such as `stim:`. Santi
stores only its bearer digest. Request identity is stable: an exact retry
returns the original receipt and a changed payload under the same key conflicts.

Turn events are consumed by global opaque cursor. Santi filters payloads to the
registered zone; cursor movement may reveal aggregate volume but never another
zone's labels or content. SSE is a lossy wake-up and cursor backfill remains the
authority.

Santi also carries one built-in inbound path. `POST /api/v1/webhooks/{name}`
accepts GitHub and Feishu events for a webhook subscribed through
`/api/v1/webhooks`, and puts each admitted event on a strand in its own zone:
`github:{name}:issue:{repo}#{number}` or `feishu:{name}:chat:{chat_id}`.
Senders are filtered by the configured allowlist. The path is inbound only:
Santi sends nothing back through it and keeps no participant or message ledger
for it. Message identity, participants and delivery remain stim's; whether
human chat should keep entering here rather than through stim is an open
product question.

## Environment and capabilities

Synchronous shells start behind an explicit environment wall. Host allowlisted
values are overlaid by product config, soul and strand declarations, then
reserved runtime variables. Unresolved references stay literal and become a
visible system message rather than leaking server environment.

When configured, Santi signs a short-lived capability immediately before one
shell effect. Claims bind issuer, audience, key, soul, strand, turn, tool call,
effect, and lifetime. The private key stays in typed server config and never
enters the shell, detached job, estate, or repository.
