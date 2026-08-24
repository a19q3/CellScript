# CellScript 0.24 Myelin Handoff

**CellScript-side contract**: implemented

**External Myelin lock adoption**: pending the final clean 0.24 release commit

## Boundary

Myelin consumes CellScript as an independently versioned compiler process. It
must not vendor the compiler or add it as a workspace member. Court-facing
compilation uses the `ckb` target and CKB-strict execution. Myelin's finite
session, committee, DA, finality, projection, and `MyelinExtended` semantics
remain Myelin-owned.

CellScript therefore does not define `myelin`, `myelin_extended`,
`MyelinExtended`, `off-chain-session`, or an equivalent target profile.

## Versioned Handoff Contract

`integrations/myelin/cellscript-0.24-handoff-contract.json` freezes the
CellScript side of the transition:

- Edition 2026 and the `ckb` target;
- metadata schemas `58/2/1/2`;
- `cellscript-entry-witness-v1` inside canonical
  `cellscript-witnessargs-input-type-v2` placement;
- no raw-witness compatibility;
- lowering-record, source-map, checker, and checker-policy identities;
- exact compiler binary, source revision/tree, artifact, metadata, profile,
  lowering-record, source-map, checker binary, and checker-policy bindings;
- the untrusted scheduler-template boundary; and
- no fallback reader or alias for the prior adapter identity.

This is a repository-to-repository coordination contract, not a runtime asset
of the published `cellscript` crate. The crates.io package therefore excludes
both this file and its repository-only conformance test.

The contract is intentionally marked `pending-external-release-pin`. An exact
40-hex source revision cannot truthfully identify uncommitted branch content.
After the 0.24 branch is cleanly committed, Myelin must update its own
toolchain lock, create fresh compiler and checker attestations, regenerate its
fixtures, and pass its production gate. CellScript does not rewrite or silently
adopt a dirty external Myelin worktree.

## Scheduler Evidence

CellScript access metadata is an untrusted template. Myelin must resolve final
conflict hashes from authenticated concrete Cells and a validated full Type
Script declaration. Binding names remain diagnostics. Scheduler plans remain
sidecars bound to raw transaction identity.

The standalone artifact checker proves only that the declared access and
lowering evidence is structurally bound to the artifact. It does not prove
that Myelin resolved a conflict key correctly or that a committee finalized a
session.

## Adoption Checklist

1. Commit and identify the exact clean CellScript release revision.
2. Replace the Myelin toolchain lock in one explicit transition; do not add a
   compatibility fallback.
3. Build and attest both `cellc` and `cellscript-artifact-checker` from that
   revision with Rust 1.97.1.
4. Compile every Myelin fixture with target profile `ckb` and verify metadata,
   compatibility profile, lowering record, source map, and artifact digests.
5. Confirm the lock rejects the former raw-witness-compatible identity and all
   digest mismatches.
6. Run the Myelin adapter, static-committee, Tendermint, court, and production
   gates. Skipped external workloads remain skipped, not passed.
