# CellScript 0.24 Development Release Notes

**Status**: implementation-complete merge candidate; `dev` and `ci` passed on
2026-08-10, while `backend` must be rerun from the clean committed tree; the
full release gate remains required before production claims

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
3. Package resolution becomes lock-authoritative: `Cell.lock` v3 records a
   manifest-bound dependency graph, exact source identities, feature/test and
   CKB-environment roots, while ordinary builds never perform mutable version
   selection.

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

## Package And Registry Evolution: Lessons From Sui Move

The package work was informed by Mysten's Sui Move `move-package-alt` design at
commit `5a9f37431c473fa2f6d49abecbcc6a6d7190f533`. CellScript adopts the parts
that reduce ambiguity in an auditable CKB compiler, while retaining a different
source, artifact, and chain model.

### Lock first; repin explicitly

The central principle is that dependency selection and compilation are
different authorities. `cellc lock`, `cellc update`, `cellc add`,
`cellc remove`, and `cellc install` may resolve mutable requirements and write
a new graph. `build`, `check`, and `test` consume only that graph. A missing
lock, changed manifest digest, missing graph edge, moved source, or changed
content hash fails closed and tells the user to repin explicitly.

`--locked` documents the intent to require the existing dependency graph.
That graph is authoritative even without the flag; the flag is useful in CI
and scripts. `--frozen` additionally implies offline operation and suppresses
all `Cell.lock` writes, including refreshed build evidence. `--offline` permits
only already materialized exact Registry/Git sources.

This follows Move's separation between resolution and pinned compilation, but
CellScript keeps build/deployment evidence in the same file. Therefore an
ordinary non-frozen build may refresh `[package.build]` and deployment facts;
it never changes dependency nodes or root edges.

### A graph, not a flat version list

`cellscript-lock-v0.24-graph-v1` records:

- the exact SHA-256 digest of the root `Cell.toml`;
- canonical dependency nodes with declared package name, SemVer, immutable
  source, whole-tree source hash, dependency-manifest digest, and outgoing
  alias-to-node edges;
- separate runtime and test root edges;
- feature-qualified node identities; and
- named CKB environment roots bound to both `chain_id` and the 32-byte genesis
  hash.

The graph allows two source/version nodes to coexist in resolution. It does
not pretend that two packages declaring the same CellScript module are safe:
the compiler's existing duplicate-module and type-identity checks still fail
closed. This is deliberately narrower than importing Move's package/type
identity wholesale.

Git branches and tags are update-time conveniences only. They resolve to a
full 40-hex commit and an immutable local cache. A later branch movement has no
effect on a locked build; only explicit repinning observes it. Registry sources
are likewise materialized from the exact snapshot URL and `sha256:` identity
recorded in the lock, without repeating discovery or version selection.

### Standard SemVer, aliases, features, tests, and environments

Version requirements now use standard SemVer matching, including correct
`0.x`, prerelease, build-metadata, range, and lower-bound behavior. A bare
CellScript version retains the existing compatible (`^`) meaning.

Dependency aliases are separate from declared package identity through
`package = "..."`. `[features]` supports `default`, feature-to-feature
expansion, and `dep:<alias>` activation for optional dependencies. Feature
cycles and unknown activation targets are rejected. `[dev_dependencies]` enter
only the `cellc test` graph. `[build.dependencies]` remains reserved and fails
closed because executing build scripts without an isolation contract would
expand the trusted computing base.

`[environments.<name>]` binds dependency choice to a concrete CKB chain
identity. `[dependency_overrides.<name>]` can replace declared dependencies for
that environment, but there is no implicit mainnet/testnet selection: callers
must pass `--environment <name>` when overrides exist. This adapts Move's named
environment idea to CKB's genesis-bound Cell Model rather than copying Sui
addresses or published package IDs.

### Bounded resolver extension, normalized before trust

`[resolvers.<name>]` is a versioned extension point for package ecosystems that
cannot be expressed directly. The executable path is absolute and SHA-256
bound. CellScript invokes it without a shell or inherited environment, with a
10-second deadline and 1 MiB stdout/stderr limits, over
`cellscript-dependency-resolver-request-v1`. The response must use
`cellscript-dependency-resolver-response-v1` and normalize to either an exact
Registry version or a Git URL plus full commit.

The resolver itself is never stored as build authority and is never executed
by a locked build. `Cell.lock` contains only the normalized source and content
identity. This preserves Move's extensibility insight without permitting an
unbounded plugin system inside compilation.

### Registry profiles are versioned and fail closed

Registry artifact admission now uses
`cellscript-registry-profile-catalog-v1`. Every supported profile names its
validator, allowed kinds/languages/consumption modes, whether a profile
contract is required, and whether it may participate in dependency resolution.
Only `cellscript_source` has `resolver_capability = dependency`; CKB
executables, reproducible builds, and copy material remain discoverable but
non-resolving. Adding a future profile is therefore an explicit versioned
contract change rather than another scattered conditional.

What 0.24 does **not** copy from Move is equally important: there is no Move
bytecode/module ID, Sui address or object identity, implicit environment
selection, unrestricted resolver plugin, source-equivalence claim from hashes,
or conversion of executable/copy artifacts into source dependencies.

## Validation

The package/Registry closure passed `dev` and `ci` on 2026-08-10, with the CI
website phase using the required Node 22 toolchain. The backend compiler,
tests, Clippy, and static audit also passed, but its stateful acceptance harness
correctly rejected the uncommitted source tree. The exact committed tree must
therefore pass the complete `backend` gate before this candidate is promoted:

```bash
./scripts/cellscript_gate.sh dev
./scripts/cellscript_gate.sh ci
./scripts/cellscript_gate.sh backend
```

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
