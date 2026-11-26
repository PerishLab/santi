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
Input sent during a Turn may join its next safe ordinary provider round, after
all current tool results are recorded. It does not interrupt an in-flight
request or grant another round: maintenance, stop, exhausted ordinary allowance,
or a batch that does not fit leaves input pending for a later eligible round
or the next Turn. Inference stays frozen for the active Turn; its execution
allowance is not reset. A `prepared` MessageEvent
with `kind: input_prepared` records selection in a prepared request, not provider
receipt or compliance.

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
resolve; `crates/santi-estate/tests/ledger/model.rs` classifies every text field.
Ending a strand through Keel removes what it owns, and its bytes on disk follow
the committed estate; the start sweep removes bytes no live row owns.
Issue [#226](https://github.com/PerishLab/santi/issues/226) records the design.
`strand end <id>` and `DELETE /api/v1/strands/{strand}` retire one explicitly
identified idle strand. Pending input, unfinished turns or receipts, active or
unacknowledged Jobs, unsettled effects and live forks block retirement in the
same transaction that ends the strand. Files follow the commit with best-effort
cleanup and startup sweep recovery; physical collection follows Keel retention.
Issue [#251](https://github.com/PerishLab/santi/issues/251) records the contract.
Retirement returns the Strand ID, time, ended Job IDs and counts of owned
Messages and Turns. Keel retains the retirement evidence under the owning Soul
in the same transaction that ends the Strand. Read it by exact ID with
`santi strand retirement <id>` or `GET /api/v1/strands/{strand}/retirement`.
Issue [#279](https://github.com/PerishLab/santi/issues/279) records this addition.

`santi query` and the structured `santi job query` shortcut are bounded,
read-only Job reads. Native Keel keeps `tag` as the public Job id and deliberately
does not add an `id` alias. A native query must declare `limit` or `count`; the
structured shortcut defaults to 20 rows. `--count` is the independent count of all
matching Jobs within the selected scope and filters, not a truncated page. `--all-strands` overrides `SANTI_STRAND_ID`; only an
explicit `--strand` with `--all-strands` conflicts. These reads never refresh
process state and leave `santi job list` / `GET /api/v1/jobs` unchanged. Issue
[#401](https://github.com/PerishLab/santi/issues/401) records the contract.

Incident lists page retained active and resolved records by first-seen time and
ID descending. Use the last returned ID as `before` to continue; new arrivals
appear on a fresh first page. Exact incident inspection includes the persisted
opened/resolved snapshots after delivery acknowledgement. Repeated reports keep
first/latest summaries and a count. History follows its owning soul or strand;
retired records are unavailable. Issue [#253](https://github.com/PerishLab/santi/issues/253)
records the read contract.

Live credentials belong in the ignored `santi.toml`; use `santi.example.toml`
for tracked configuration changes.

`santi inference sources` manages provider, family, access, alias, offering,
scenario, candidate and preference records in the Santi estate. Use `list`
and `explain PROVIDER FAMILY SCENARIO` to inspect the current choice. Create
takes a JSON definition, update takes `expected` and `replacement`, and
delete takes the last observed record. Conflicting revisions and referenced
resources report the reason. HTTP mutations take the operator token from
the client's `operator` file, checked against the service's `server.operator`
file. Provision the same random token (at least 32 characters) in private
files for the two operators. Access definitions carry only a `custody://`
reference to a transport credential.

Service, Soul and Strand inference declarations select a scenario by its full
path, for example `teamorouter/gemini/recall`. The supplier and family each
occupy one lowercase ASCII word. The scenario occupies one to three lowercase
words separated by dots. Look up the exact path and its preferred candidate.
The offering supplies the wire model and the access identifies its transport.
Reasoning effort can be inherited or overridden independently. Each admitted
turn records its actual choice. Older turns retain the facts they recorded.

Keep credentials and transport policy in operator custody. Endpoint and
protocol must match the access declaration. Chat and Gemini profiles can
retain their admitted continuation identity. Request-header templates require
a private policy seal held with the transport configuration for recovery.

Qualify a new offering in an isolated instance: select the candidate in that
instance, run the scenario criterion, and inspect its admitted turn. Compare
the evidence for each candidate under the same criterion before changing the
operating preference. See crates/santi/INFERENCE-TRANSFER.md for the operator
commands.

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
`crates/santi-core/src/assembly/skill.rs` owns on-demand operating guidance;
Issue [#242](https://github.com/PerishLab/santi/issues/242) records its role in
Harness orientation and learned-memory correction.
Material means metadata plus blobs assembled into context, with explicit revisions,
bindings and consumption records. Bucket metadata belongs to the object ledger;
memory reads and rendered inspection views have their own types and data flow.
The old strand material POST and CLI --kind form remain explicit render adapters
with their existing wire behavior; they do not mutate material bindings. Keel
owns the one-time bucket ledger resource rename and identity preservation.
Issue [#266](https://github.com/PerishLab/santi/issues/266) records this boundary.

`crates/santi-core/src/library` resolves configured material sources into
content revisions; #256 records source/query semantics. Published PerishLab
Skill directories retain their upstream managed shape; source provenance is
not proof of remote authenticity. #257 owns durable Soul/Strand bindings and
per-request material selections. Bound revisions retain their bytes in the
estate, independent of source availability. Composition, estimate and request
capture share one selection; binding changes apply to the next provider round.
Drop removes an own binding, and compact retains bindings. Query/drop remain
available through the material tool during maintenance.
`crates/santi-core/src/service/flow/memory.rs` owns memory pressure
and maintenance.

Compact ranges retain exact timeline boundaries. New Compact head/tail relations
name StrandEntry rows; legacy first/last Message relations remain readable.
first/last selectors accept fixed projected Messages and proven complete exchange
anchors/results. from/to retain Message-sequence meaning. Default selection keeps
the latest Message and the entire active exchange live; it can compact an earlier
tool-only exchange without fabricating assistant text. Selection, preview and
commit share exchange safety checks. Query, occupancy, projection and fork use
the same entry range. The visible covered_entry_range names these record IDs;
covered_message_range remains its historical wire alias. Issue #274 owns this
contract and its estate-continuity checks.

For execution and recovery changes, follow
`crates/santi-core/src/service/flow` into `crates/santi-estate/src/store`.
Receipt, completion, and effect behavior is exercised in
`crates/santi-estate/tests/completion`,
`crates/santi-estate/tests/operations/terminal.rs`, and
`crates/santi-estate/tests/effects`.

`santi skill --help` owns managed CLI guidance. Plumb owns its installation
ledger and version binding; Depot holds the singleton `SKILL.md` generation
for each release. Runtime Harness guidance remains a separate authority.

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

For a real-provider turn, use private custody and a qualified scenario in the
chosen test estate. Start `santi-api --config santi.toml serve` for that
estate and point the client at its listener, then:

```sh
SID=$(santi strand create | jq -r .strand.id)
SANTI_STRAND_ID=$SID santi strand send 'Reply with exactly: OK' --watch
```

Inspect an accepted send with `santi receipt <inbox>` and
`santi strand runtime <strand>` when its watch result remains unknown.
Stop the server after the check.

Feishu event IDs use the existing durable delivery gate; their decrypted envelope
fingerprint detects conflicting reuse. Optional webhooks.feishu.acknowledge names
one native Runseal profile. After durable intake, the adaptor reserves one
receipt-owned acknowledgement and dispatches the case-sensitive Get emoji with
explicit bot actor. Acceptance returns independently of that outward result.
The acknowledgement inspection API retains intent, state and complete process
result separately from task receipt/completion. An interrupted dispatch remains
uncertain and is never automatically replayed. Observe the exact message's
reactions through native Runseal before explicit recovery. A confirmation failure
never authorizes re-running the task. Start/progress outside the explicitly enabled receipt reply window remains consumer policy. Issue #304 owns this boundary.

Unknown acknowledgements block Strand retirement. Startup marks interrupted
dispatches unknown without replay. After exact platform observation, the explicit
confirmation endpoint records operator judgment separately from the original
result; only then can that uncertainty settle. It never calls the platform.

Current execution activity has its own latest-value observation contract.
`Service::observe` and the Strand `/activity` and `/activity/events` endpoints
share process-local epoch/revision snapshots. Active executions retain their
latest phase even without subscribers; intermediate updates may coalesce.
A null current activity does not establish Strand idleness, task completion,
or a failed Turn. Exact receipt/turn records remain authoritative for results.
Round zero is preparation before the first provider round. Responding means
provider activity was observed; invoking means a tool invocation is being
handled, not proof a subprocess started. Activity includes no reasoning text.
Old execution/round updates and cleanup cannot replace newer activity. Reopen
starts a new epoch without replay; consumers correlate exact turn/round and
input-selection evidence rather than assuming a new input joined the active
request. Issue #376 records this boundary; #373 owns its Feishu projection.

Feishu feedback is independently enabled with webhooks.feishu.feedback (native
Runseal profile) and application (expected App ID). Each accepted receipt owns
one Window in the same intake transaction; its bounded record holds only external
channel bindings, finite intents and results. It contains no estate row tags.
The first native post link uses the fixed a.href
https://github.com/PerishLab/santi#feishu-feedback-v1; final soul content omits it.
A delayed coarse receipt-state line is sufficient for v1. Completed means the
runtime round ended, not business completion; a fast completed receipt creates
no stale automatic bubble. Fine phases require exact input/turn/round evidence
from the runtime observation contract, never reconstruction from logs.

Use the exact inbox receipt and current strand for `santi reply`. Soul ownership
is one-way, cancels queued automatic intent and waits for the one dispatched
operation. Reply creation can settle after takeover; its exact message identity
then becomes the final edit target. Same operation/payload returns prior intent;
a second final operation refuses. Unknown and interrupted writes freeze every
later dispatch and block retirement. Explicit resolution requires evidence that
the original remote request settled; current body alone cannot rule out a late
write. Original process results and separate resolution remain retained. No
fallback bubble, automatic replay or task rerun follows a channel failure.

Adaptor-owned Get and feedback writes also share one process-lifetime Radicula
factory keyed by the explicit Runseal profile, keeping their credential sessions
from contending with each other. A feedback operation remains queued while
waiting for this gate, so soul can still cancel an unsent automatic intent.
Receipt serialization remains the durable platform-ordering authority; the
local gate grants no remote fencing or coordination with unrelated processes.

A Window's optional input relation is attached in the same drain/absorb transaction
that creates its Message. Provider assembly batches those relations for the
current Strand and projects `[reply INBOX]` after that Message's normal header.
Raw Message bodies stay unchanged; preview and budget use the same projection.
Forks do not inherit their parent's reply authority. Soul need not reconstruct
receipt identity from event payloads or select the latest message.
