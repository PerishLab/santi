# Transfer inference sources

```
santi inference sources import legacy.toml --mapping map.toml --dry-run
santi inference sources import legacy.toml --mapping map.toml --apply
santi inference sources export > catalog.json
santi inference sources import catalog.json --dry-run
```

For each `[providers.NAME]` in the service TOML, declare one entry in `map.toml`:

```toml
[profiles.old_gateway]
provider = "gateway"
access = "gateway-chat"
custody = "gateway-operator"      # name only, not a credential
aliases = ["old_gateway"]         # retain declarations using the old access name
offerings = { "vendor/model-a" = "family-a", "vendor/model-b" = "family-b" }
```

Use `aliases = []` if the access keeps the original name; `offerings = {}` is valid
if there are no models to map. If a legacy profile default is private, use
`default_in_custody = true` and omit it from `offerings`: the operator profile,
not the catalog, supplies that default. Only explicitly approved public model strings
become offerings. Profile defaults and service/soul model declarations are
checked for coverage, not applied as defaults or used to infer families.
Only the protocol and public endpoint are projected from a service profile.
No environment references are resolved. Credentials, request/response headers,
limits, defaults, and other execution options remain operator configuration.
Keep the original Profile under `[custody.gateway-operator]` on the service;
provision the same custody name with the matching protocol/endpoint separately
on each destination. A binding present in the input is checked locally. No
command installs or tests credentials, or contacts an inference provider.

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
