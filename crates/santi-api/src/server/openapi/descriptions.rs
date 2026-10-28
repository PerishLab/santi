pub const COMPONENT_DESCRIPTIONS: [(&str, &str); 12] = [
    (
        "budget.Execution",
        "Execution policy for a strand. `timeout` is the foreground shell deadline in seconds, including waiting for inherited stdout/stderr pipes. A deadline interrupts the managed process group and reports a tool failure; partial external effects remain possible. Detached Jobs own long waits and their separate timeout policy.",
    ),
    (
        "render.Kind",
        "Legacy render selector: system_prompt returns the assembled runtime prompt; skill returns Santi operating guidance. Rendering does not change material bindings or memory.",
    ),
    (
        "Soul",
        "A soul is a cyber-individual, keyed by id alone. It has no name/avatar/desc\ncolumn: identity is the mutable self, and it lives entirely in the soul's\nmemory (rendered live into `[santi-soul]`), not in a profile row. The\ntimestamps are pure provenance.",
    ),
    (
        "soul::Draft",
        "Create a new soul (an individual). Souls are API-managed, never config.\nA soul is id-only; its identity is its memory, so the only thing to supply\nat creation is the initial `[santi-soul]` memory to seed (empty/absent → a\nblank soul that will author its own). The exact requested UTF-8 bytes are\natomically published before the soul becomes list/get-visible. Database and\nfilesystem publication are ordered, not falsely claimed as one transaction;\na failed creation may leave unlisted filesystem residue.",
    ),
    (
        "webhook::Subscription",
        "An API-managed webhook subscription: how an external source reaches a soul.\n`adaptor` selects the boundary normalizer (integration knowledge); `soul`\nis who receives the resulting turn; `strategy` picks where the thread\nlives (`per_thread` = one strand per adaptor-derived label, `single` = one\nstrand per subscription); `credential` names the env var holding the signing\nsecret (the secret itself is never stored). The `name` is the URL path segment.",
    ),
    (
        "webhook::Draft",
        "The complete desired webhook subscription. POSTing it is an idempotent\nensure: an absent name is created, an identical subscription is returned\nunchanged, and drift under an existing name is rejected with 409.",
    ),
    (
        "receipt::State",
        "Current durable responsibility state for one accepted inbox item. A\nmechanically-recovered transition can be immediately followed by `driving`\nin the same transaction; callers inspect `transitions` for that evidence.",
    ),
    (
        "compact.Compact",
        "A compact is a pure projection overlay over a strand's spine. It\nself-describes its coverage by message-id boundaries and carries the\nsupplied summary while originals remain queryable. Manual compact metadata\nrecords precommit estimates only under `forecast`: `authoritative` is `false`\nand `basis` is `precommit_preview`. The provider-visible header keeps estimated\npost-context bytes and ratio under\n`context_estimate.forecast`; they are not exact postcommit context.\n`context_estimate.availability` distinguishes estimated values, not-recorded\nlegacy data, unavailable estimates and unbounded configuration; unavailable\nnumeric fields are omitted rather than presented as zero.",
    ),
    (
        "effect::State",
        "Durable truth for one concrete external-effect attempt. It is deliberately\nnot turn state: one turn may contain several independently settled or\nambiguous effects.",
    ),
    (
        "job::Job",
        "A soul-owned detached command with an execution lifecycle independent of\nits creating shell invocation, tool call, effect, turn, and receipt. Origin\nfields are immutable provenance and never imply cascading state.",
    ),
    (
        "job::State",
        "`accepted` means the detached sidecar durably published its initial\n`claimed` state, not that the payload started. `unknown` means retained evidence\ncannot prove either a live matching stamp or a terminal outcome; Santi never\nreplays it automatically.",
    ),
    (
        "message::Role",
        "No user/account actor: santi is individual-first, not multi-tenant. All\ninbound (a CLI send, a webhook event) arrives as `System` — the sender's\nidentity is metainfo carried in the content, opaque to core, not a distinct\nactor kind. `(actor, kind)` is the full marker at the provider\nboundary (see `item`): Soul→assistant, System+Text→user\n(world-inbound), System+SantiSystem→system (runtime-meta, not user speech).",
    ),
];

pub const PROPERTY_DESCRIPTIONS: [(&str, &str, &str); 14] = [
    (
        "material.Component",
        "bytes",
        "UTF-8 bytes of this rendered material fragment, including metadata and delimiters; distinct from provider JSON estimates and tokens.",
    ),
    (
        "material.Context",
        "thinking",
        "Observed provider span. Null means prepared without an observed span; a record does not establish model compliance.",
    ),
    (
        "downstream::Draft",
        "digest",
        "Lowercase or uppercase SHA-256 hex of a high-entropy Bearer token. The digest is stored but never returned.",
    ),
    (
        "ingest::Request",
        "request",
        "Stable idempotency key, unique within the authenticated downstream. Reuse with a changed payload is a conflict.",
    ),
    (
        "event::Batch",
        "cursor",
        "Opaque global high-water mark. Persist it even when events is empty; it reveals aggregate activity but no foreign payload.",
    ),
    (
        "Health",
        "incidents",
        "Aggregate only: `/health` is public and must never expose strand,\nreceipt, or incident locators.",
    ),
    (
        "Strand",
        "label",
        "Opaque external anchor (e.g. a webhook thread key). Unique per soul;\nabsent for strands reached only by id (e.g. CLI-created ones).",
    ),
    (
        "webhook::Draft",
        "strategy",
        "`per_thread` (default) or `single`.",
    ),
    (
        "strand::Posted",
        "message",
        "Absent at durable acknowledgment. Processing drains the accepted inbox later;\nquery the exact receipt or strand runtime to follow its message and turn.",
    ),
    (
        "receipt::Status",
        "effects",
        "Per-attempt shell effects reached by any turn carrying this receipt.\nCompletion alone does not imply that any listed external effect applied.",
    ),
    (
        "effect::Effect",
        "call",
        "Absent only for an imported legacy row whose old schema had no neutral\ntool-call locator.",
    ),
    (
        "effect::Status",
        "receipts",
        "Obligation roots whose attempts include this effect's turn.",
    ),
    (
        "job::Accepted",
        "job",
        "The accepted resource. Its current state may advance quickly, but create\nsuccess promises that the detached sidecar durably claimed execution responsibility.",
    ),
    (
        "job::Log",
        "next",
        "Opaque monotonic byte cursor for the next read of this stream.",
    ),
];
