# Registry Artifact Verifier

This is the least-privilege `ckb_executable` admission worker. Its normal
dependency graph contains `cellscript-artifact-checker` and does not contain
the CellScript compiler.

The worker accepts one path-confined Registry bundle and verifies its
coordinate, canonical manifest, and declared hashes. Generic source/executable/
ABI bundles remain `hash_bound`. If any CellScript verified sidecar is present,
the worker requires the complete metadata/lowering-record/source-map set and
runs the standalone checker. Successful structural JSON records
`structurally_verified`, checker version, checker policy schema, and a hash of
the canonical checker report.

For `cellscript-registry-ls-idl-interface-v1`, the same worker independently
validates the bounded LS-IDL schema, hashes the exact ABI object bytes with
SHA-256, and requires that digest as the executable's final 32 bytes. Its
result records the interface format, digest, and
`schema-and-suffix-bound` status. That status is byte-identity evidence, not an
implementation-correctness or security-audit claim.

The root gate proves the production dependency boundary with `cargo tree`.
The root compiler is present only as a dev-dependency so integration tests can
construct a real valid bundle; it is not linked into the production binary.
