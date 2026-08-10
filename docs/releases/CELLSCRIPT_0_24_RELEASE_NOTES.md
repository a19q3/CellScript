# CellScript 0.24 Development Release Notes

**Status**: merge candidate; `dev`, `ci`, and `backend` passed on 2026-08-10,
and the full release gate remains required before production claims

**Source edition**: 2026

**Metadata schemas**: 58 / 2 / 1 / 2

**Rust toolchain**: 1.97.1

## Highlights

The 0.24 line closes two trust gaps without adding a new language edition:

1. CKB ELF builds emit a canonical verified lowering record and source map,
   and a smaller compiler-independent checker recomputes bounded structural
   invariants over those sidecars and final machine bytes.
2. `cellc test` requires an execution backend and runs versioned positive,
   exact-negative, and multi-step local Cell scenarios under the simulator,
   CKB-VM, or both.

The Registry can now admit artifact-only CKB bundles through a least-privilege
worker that depends on the standalone checker, not the CellScript compiler.

## Verified Artifact Files

ELF builds add:

- `ARTIFACT.lowering.json` using
  `cellscript-verified-lowering-record-v1`;
- `ARTIFACT.sourcemap.json` using
  `cellscript-source-artifact-map-v1`; and
- a `verified_artifact` identity in metadata schema 58.

`cellc verify-artifact` reports binding, structural, lowering-record, CKB-VM,
and chain evidence independently. Successful checking is not described as
complete source equivalence, VM execution, deployment, or commitment.

## Checker Evidence

The standalone checker validates canonical JSON, budgets, graph and
reachability policy, typed ABI/frame contracts, ProofPlan links, static ELF
shape, emitted RV64 instructions, canonical call targets, control flow, stack
restoration, syscalls, block digests, and source-map ranges. Stable rejection
codes `V2400` through `V2418` have a deterministic mutation corpus.

The production dependency graph of both the checker and the Registry
artifact-only verifier excludes the CellScript compiler. The Registry records
the checker version, policy schema, and report hash for structurally verified
admission.

The checker is packaged as an independent crates.io dependency. Packaging
gates verify it first and then verify the compiler against an exact local
registry patch; an actual release must publish the checker before the compiler.

## Executable Package Scenarios

`cellc test` requires `--backend simulator|ckb-vm|all` unless `--no-run` is
used. Scenario JSON rejects unknown fields and validates confined source paths,
named live Cell replacement, script identities, deps, headers, `since`,
witness fields, capacities, limits, and exact runtime errors.

Simulator output is development/non-consensus evidence. CKB-VM output is local
authoritative runtime evidence, not chain evidence. The v1 CKB-VM runner
supports no-argument entries; transaction-syscall cases remain with the
stateful CKB oracle.

Native `cellc run` now includes the VM runner by default. It executes only a
no-argument standalone ELF and fails closed for parameter or transaction/
syscall context. Development interpretation requires explicit `--simulate`;
there is no silent evidence-tier fallback.

## Integration Status

- The CellScript side of the Myelin 0.24 handoff is versioned and tested. The
  external Myelin lock update remains pending until this branch has a clean
  exact release revision. No raw-witness alias or Myelin target profile is
  added.
- Fiber remains no-profile. Static compiler/CKB-VM evidence is retained, but
  the complete external lifecycle and negative matrix has no complete evidence
  bundle and remains pending.
- RGB++ remains an ecosystem identity sidecar. Rgbpp Lock, BTC Time Lock, BTC
  SPV, witness/commitment, deployment, confirmation, reorg, and paired
  CKB/Bitcoin evidence are not complete and are not promoted.

## Validation

The merge-readiness gates passed on 2026-08-10:

```bash
./scripts/cellscript_gate.sh dev
./scripts/cellscript_gate.sh ci
./scripts/cellscript_gate.sh backend
```

The clean-snapshot full backend audit produced
`strict-backend-audit-full-20260810-023933.json`; the final in-tree CI audit
produced `strict-backend-audit-ci-20260810-025025.json`.

`release`/`release-quick` still require the pinned CKB, CKB SDK, NovaSeal,
Docker, Node 22, and RISC-V tooling described in the gate policy. Passing the
three merge gates is not a substitute for the release gate or public-chain
evidence; neither release mode has been run for this merge candidate.

## Detailed References

- [Verified artifact boundary](../CELLSCRIPT_VERIFIED_ARTIFACT_BOUNDARY.md)
- [Executable test scenarios](../CELLSCRIPT_EXECUTABLE_TEST_SCENARIOS.md)
- [Myelin handoff](../CELLSCRIPT_MYELIN_0_24_HANDOFF.md)
- [Gate policy](../CELLSCRIPT_GATE_POLICY.md)
- [0.24 roadmap](../../roadmap/CELLSCRIPT_0_24_ROADMAP.md)
