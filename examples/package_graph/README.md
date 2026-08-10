# Lock-Authoritative Package Graph

This portable 0.24 example concentrates the package features that do not
belong in the business-contract examples:

- the local alias `core` resolves declared package `canonical_math` through a
  standard `^1.2.0` SemVer requirement;
- optional `audit_helpers` is activated through `dep:audit` and the transitive
  `full` feature;
- `scenario_test_support` enters only the test graph;
- mainnet and testnet roots bind explicit CKB chain identities; and
- the testnet environment replaces `network_contracts` with an exact `2.0.0`
  path source.

The tracked `Cell.lock` contains every feature, test, and environment root, so
the following commands perform no mutable dependency selection:

```bash
cd examples/package_graph
cellc check --frozen --offline --environment mainnet
cellc check --frozen --offline --environment testnet --features auditing
cellc test --no-run --frozen --offline --environment testnet --all-features
```

Omitting `--environment` fails closed because this manifest has an explicit
environment override. Run `cellc lock` only when intentionally repinning the
graph.

Git and Registry requirements normalize to immutable commits or snapshots at
repin time. They are not included here because a portable checked-in example
must not depend on mutable network discovery. Hash-pinned external resolvers
remain test/documentation fixtures because their commands must use
machine-specific absolute paths.
