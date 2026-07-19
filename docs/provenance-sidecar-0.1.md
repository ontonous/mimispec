# Experimental Provenance Sidecar 0.1

> Schema: `mimispec.provenance/0.1`. This protocol is outside MimiSpec Core and
> outside the 0.3 API freeze. It is not Evidence of implementation correctness.

A sidecar relates one exact MMS revision and one external source revision. All
hashes are lowercase SHA-256. Each link contains an optional source symbol/span,
one semantic `SlotLocator`, one of `observed_from`, `inferred_from`, or
`confirmed_against`, and an explanatory note.

`SlotLocator` contains:

- normalized container scope path;
- semantic node kind;
- commitment anchor kind and footprint;
- the slot ordinal within its owner;
- SHA-256 of the slot's protected text.

It deliberately excludes `SourceNodeId` and `CommitmentSlotId`, which are local
to one parsed revision. Reordering a uniquely named Fragment can still resolve;
duplicate indistinguishable scopes/slots produce ambiguity and never fall back
to a numeric ID.

```bash
mimispec provenance check path/to/file.provenance.json \
  --source-root /explicit/project/root
```

Every `mms.path` and `source.path` must be relative to `--source-root`.
Absolute paths, `..`, missing files, and symlinks that resolve outside the root
are rejected. The command reads and hashes files, parses the MMS with the
canonical parser, validates source spans and locators, and reports drift. It
does not execute MIMI, a compiler, Z3, tests, or generated code, and it never
changes a commitment suffix.

Reference fixtures:

- `docs/corpora/mimi-kv-real-project.provenance.json`;
- `docs/corpora/mimichat-real-project.provenance.json`.

The JSON schema is `docs/schemas/provenance-v0.1.schema.json`.
