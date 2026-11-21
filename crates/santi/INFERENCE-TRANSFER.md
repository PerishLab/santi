# Transfer inference sources

```
santi inference sources import legacy.toml --mapping map.toml --dry-run
santi inference sources import legacy.toml --mapping map.toml --apply
santi inference sources export > catalog.json
santi inference sources import catalog.json --dry-run
```

For each `[providers.NAME]` in the service TOML, declare one entry in `map.toml`:

```toml
[profiles.legacy]
provider = "gateway"
access = "chat"
custody = "chat"      # name only, not a credential
offerings = { "vendor/model-a" = "alpha", "vendor/model-b" = "beta" }
```

Provider and family names each use one lowercase ASCII word. A scenario name
contains one to three such words joined by dots. Keep the wire model ID exactly
as supplied. A service, Soul or Strand declares a full path such as
`gateway/alpha/recall.correction`; a missing preference fails selection.
Completed turns retain the model and route admitted at the time.

In client TOML, replace the old `[inference]` provider and model fields with
`scenario = "gateway/alpha/recall.correction"`, or remove the override to
inherit. For CLI writes, clear `SANTI_INFERENCE_PROVIDER` and
`SANTI_INFERENCE_MODEL`; use `SANTI_INFERENCE_SCENARIO` for a client default.
Use `inference strand set ID --clear scenario` to return a Strand to
inheritance while retaining its independent reasoning effort.

Legacy profile names identify the mapping source. The optional `aliases` list
carries catalog metadata. Ordinary inference declarations select a scenario.
An empty `offerings` map is valid for a transport with no exposed models.
The legacy mapping option `default_in_custody = true` skips coverage of an
unpublished model default during import. Register an offering for each wire
model used by a scene. Its preferred candidate's offering supplies the model
at admission. Service and Soul model declarations in legacy inputs are checked
for coverage during import.

The transfer projects the protocol, public endpoint and custody handle from
the legacy profile and mapping. Provision the corresponding destination
transport separately under `[custody.chat]` in this example. Keep its credentials,
headers, limits, protocol and endpoint, and remove the legacy `model` field.
The CLI checks local input bindings without resolving environment references or
contacting the inference provider.

The version-1 JSON archive carries native public fields with name references
instead of estate IDs. Offering references are `[access, model]`; candidate
references are `[access, model, scenario]`. Scenario criteria and candidate
evidence are public catalog text: do not store private data there.

Use `santi inference sources get KIND ID` to inspect one destination record.
The scenario explanation includes the access, model, protocol and evidence for every candidate.

Both modes read the destination and check the whole graph. Dry-run needs no
catalog write token. Apply uses `SANTI_OPERATOR` only for missing
records, POSTs dependencies first, and resolves their IDs from the destination.
Identical records are skipped; conflicting definitions stop the import rather
than replacing anything. A refused or uncertain write is not rolled back or
automatically retried. Inspect a fresh dry-run, reconcile any manual changes,
then apply again. Individual creates are native transactions, not a batch lock:
coordinate concurrent renames, deletes and other edits with the operator.

## Compare candidates

Use an isolated service with its own estate, custody and operator token. Point
the CLI at that instance throughout qualification. Query its local candidate
IDs and criterion before setting the preference for the trial.

```sh
santi --base-url "$trial_url" inference sources explain gateway alpha recall.correction
santi --base-url "$trial_url" inference sources list
```

With an existing preference, read that record from the trial catalog and use it
as the expected value. Set `preference_id` and `candidate_id` from those reads.

```sh
santi --base-url "$trial_url" inference sources get preference "$preference_id" |
  jq --argjson candidate "$candidate_id" '{expected: ., replacement: {kind: "preference", candidate: $candidate}}' > trial.json
santi --base-url "$trial_url" inference sources update trial.json
```

For a first preference, create it in the trial catalog using
`santi --base-url "$trial_url" inference sources create preference.json` with
`{"kind":"preference","candidate":ID}`, substituting the numeric candidate ID.
Use a fresh trial Strand with equivalent starting context and the same task
for each candidate. Inspect the route of the turn identified by the receipt,
compare the response against the criterion, and record the evidence with the
candidate. Promotion uses the destination catalog's own IDs.

```sh
santi --base-url "$trial_url" inference strand set "$strand" --scenario gateway/alpha/recall.correction
santi --base-url "$trial_url" inference strand get "$strand"
santi --base-url "$trial_url" strand send --watch "$strand" "$trial_task"
```
