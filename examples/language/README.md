# Language Examples

The checked-in language examples are organized by the behavior they teach or
verify. Directory names and `.cell` filenames are semantic and stable across
release lines:

| Directory | Scope |
|---|---|
| `core/` | Canonical syntax, formatting, and standard-library surface |
| `ckb/` | CKB syscalls, sources, hashes, capacity, time, and TYPE_ID behavior |
| `ownership/` | Linear ownership, borrowing, and resource lifecycle |
| `verification/` | Invariants, proof-plan behavior, and transaction views |
| `collections/` | Bounded collection and local collection patterns |
| `batches/` | Dynamic bounded Cell sets, output plans, and atomic batch flows |

`ownership/semantic_foundation.cell` is deliberately valid under both the
stable 2026 frontend and the experimental 2027 frontend. It demonstrates
explicit input/witness provenance and successor-based Cell disposition. Run
`cellc expand` in a package containing the source to inspect the canonical
semantic foundation; the human rendering is diagnostic and is not a hash
boundary.

For the canonical native spelling, use the locked
[`examples/semantic-foundation-2027`](../semantic-foundation-2027/README.md)
and [`examples/lock-script-2027`](../lock-script-2027/README.md) packages. They
are intentionally bounded Edition 2027 previews rather than stable-language
examples.

Do not put a release or version number in a `.cell` filename. Names such as
`v0_26_batch.cell`, `contract-v1.cell`, and `token_0.26.0.cell` are rejected by
the native source-policy gate. Describe the example's behavior in its filename
and record release history in `CHANGELOG.md` or release notes instead.

These examples exercise compiler and tooling surfaces. Unless a document says
otherwise, they are not part of the production CKB acceptance matrix.
