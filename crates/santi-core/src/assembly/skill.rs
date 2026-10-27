pub(crate) const TEXT: &str = "\
---
name: santi
description: Operate the current Santi runtime as one soul with multiple strand brains.
---

# Santi

Santi keeps individuals and their durable execution. Start with your current
identity and the runtime's observations; retained experience carries its own
source, time and conditions.

## Locate

The current invocation supplies SANTI_SOUL_ID and SANTI_STRAND_ID to `santi`.
SANTI_TURN_ID, SANTI_TOOL_CALL_ID and SANTI_EFFECT_ID identify that invocation;
SANTI_JOB_CREATE_CAPABILITY is a one-use Job creation grant. Inherited history
may describe another invocation. Use your current scope when acting.

The shell's default cwd is the configured execution workspace. `soul://` and
`strand://` select actual scoped workspace directories for shell cwd. Their
source locations are SANTI_SOUL_MEMORY_DIR and SANTI_STRAND_MEMORY_DIR.
MEMORY.md is editable source; the prompt contains live, bounded projections of
it. A blank memory is a valid starting state. Inspect sources when needed.

## Find the interface

`santi --help` and subcommand help own CLI grammar. `santi schema` retrieves the
running service's OpenAPI contract, including HTTP operations without a CLI.
Save a large response and inspect the relevant operation rather than dumping
the whole history or contract into a tool result. Read operation descriptions,
scope and authentication requirements; method names and schema shape alone do
not establish behavior. The CLI supplies scope for its typed operations.

`santi strand material --kind system_prompt` is the legacy render adapter for
the assembled system prompt. `--kind skill` renders this guide. These inspection
views do not load or drop material, or change memory or instructions.

## Bind material

Material is metadata plus declared blobs, assembled into context from a pinned
revision. `santi soul material` and `santi strand material` expose query, load
and drop. Bucket objects are files with a separate metadata ledger; memory is
editable source, and rendered views are inspection snapshots.
Use `query --available` to discover configured sources and `query <reference>`
to inspect metadata; add `--blob <name>` to read a declared blob. `load <reference>`
pins a complete revision into that scope. Repeating load explicitly refreshes it;
source edits alone do not change a binding. `query` lists own bindings and the
effective set; `--effective` includes both scope origins. Equal references and
revisions render once. Different revisions across scopes refuse with a conflict.
Drop removes only that scope binding; a shared Soul binding can remain active.
Neither action rewrites memory or revokes already learned experience.

Changes take effect at the next provider round, including later rounds of this
turn; an in-flight request keeps its original selection. Fork copies current
Strand bindings and shares Soul bindings. Compact preserves bindings. Query
`--context all` lists durable request selections, including preparation without
an observed response. Query a context or thinking ID with `--context <id>` to
inspect that exact selection; add reference and blob to retrieve retained bytes.
This proves runtime assembly, not model obedience. Budget reports separately
identify rendered material UTF-8 bytes and provider projection estimates.

Use `--operation <id>` on load/drop when tracking an operation. After uncertain
transport, `query --operation <id>` retrieves its committed result; reusing the
ID with changed content refuses. An old successful operation never reapplies
after newer changes. During context maintenance, the material tool can query or
drop persisted bindings even when ordinary shell execution is unavailable.

## Observe and act

Use strand runtime for durable history, execution-tail for bounded recent
records, budget for current estimates and limits, and errors for incidents.
One strand sequence space includes messages, thinking, calls and results.
Gaps in a messages-only view alone do not demonstrate missing history.

An accepted send names one durable inbox receipt. `send --watch` succeeds only
after that exact receipt reaches completed; it does not prove strand idleness
or settlement of every external effect. Unknown delivery is not permission to
resend. Inspect the receipt and underlying state before explicit recovery.
Calls record requests, results record returns, and effects record external
attempts. A settled effect can be applied or not_applied. Unknown outcomes need
evidence before resolution or retry; a stopped command may have partial effects.

Use shell for short work and detached Jobs for long or unknown waits. Jobs have
their own lifecycle and bounded logs. Use alarms for future attention. Bound
network observations by time: SSE subscriptions remain open, and limiting
output bytes does not bound waiting. Consult current help and budget policy.

## Maintain continuity

You can compact your own settled context. Supply a useful summary with recall
cues, sources and uncertainty; originals remain queryable. Follow the compact
projection's current retrieval command. Budget and slot reports describe
different parts of context maintenance; inspect their meanings before inferring
limits from field names or one sample.

Fork creates another brain of the same soul with a complete history prefix.
Its returned fork position is the actual inherited boundary; active or failed
execution may be excluded. Strand workspace files are copied in their current
state, soul memory remains shared, and new execution has its own turns/effects.
Send an explicit task, follow its receipt, and retrieve the brain's reply or
state. Avoid waiting for work that requires your own active turn to finish.

Keep enduring personal understanding in soul memory and local evidence in
strand memory. Separate observations, interpretations and unknowns, retaining
sources and applicable conditions. Recheck learned claims against current
authoritative evidence and revise them when contradicted. Experience guides
judgment; it is not a second source of current runtime truth.
";
