# CellScript 0.24 Roadmap

**Status**: Core implemented and merge gates passed on `nightly-0.24`; external
Myelin lock adoption and conditional Fiber/RGB++ evidence remain pending

**Theme**: independently verified artifacts, executable package evidence, and
bounded runtime integration

**Depends on**: Edition 2026, metadata schema 58, the resolved compatibility
profile, canonical `WitnessArgs.input_type` placement, the native
`cellscript-tools` gate, the public Registry verification worker, the existing
CKB-VM acceptance harnesses, and Myelin's external compiler-process adapter

## Goal

0.24 should reduce the amount of CellScript that a consumer must trust without
pretending that an untyped RISC-V ELF has the same verification surface as a
typed virtual-machine bytecode.

The release has two mandatory outcomes:

1. a small, bounded checker independently validates a stable lowering record,
   its metadata claims, and the structural CKB RISC-V artifact contract; and
2. `cellc test` executes package-authored positive and negative scenarios and
   can promote selected cases to authoritative CKB-VM evidence.

Source-to-artifact maps connect those outcomes. They let the checker, test
runner, trace tools, Registry worker, and auditors refer to the same action,
lock, basic block, ProofPlan obligation, runtime error, and instruction range.

The release also completes the safe integration handoff that 0.23 originally
described too broadly. Myelin remains a separate finite-Cell session runtime.
It consumes the upstream compiler and the independent checker through an
attested process boundary; `MyelinExtended` remains Myelin-owned semantics and
does not become a CellScript target profile. Fiber and RGB++ promotion remains
evidence-gated and cannot turn an incomplete external matrix into a compiler
claim.

An additional delivered Registry slice makes LS-IDL a first-class interface
for deployed CKB Lock Scripts. The profile preserves exact upstream IDL bytes,
binds their SHA-256 to the executable suffix, validates them in both Registry
verifier boundaries, and resolves them by chain-verified Script identity. It
also pins the complete current client vectors and derive/example IDLs and
provides an opt-in direct test in which the upstream Rust client calls the
Registry compatibility route. The website names this surface `LS-IDL`
explicitly and aligns it with the full-width Browse surface. This does not
expand the language edition or claim implementation correctness.

The mainnet and Pudge Testnet Registry sites also share one versioned interface
contract. Both builds must expose the same six Registry routes and load the
same byte-identical visual and interactive assets. Only network authority and
network-derived state may differ: API/static origins, address prefix, chain,
sandbox expiry and indexing policy, and the records admitted to each isolated
store. Testnet lookup, API examples, Manage defaults, and artifact fallbacks
must never silently select mainnet.

## Why This Is The Next Boundary

CellScript 0.23 completed an operational distribution and evidence layer:
Edition 2026 identities, canonical entry placement, the public Registry,
compiler-backed source-package verification, reproducible artifact evidence,
native gate tooling, and bounded CKB/Fiber evidence. The remaining trust gap is
not another syntax feature. It is that the compiler still creates most of the
facts later consumed by `verify-artifact`.

Sui Move provides a useful comparison, but not a design to copy literally. Its
typed bytecode is independently checked for control-flow, stack, type,
resource, reference, and platform-specific object rules, and the verifier
itself is metered. See the pinned upstream
[Move bytecode verifier contract](https://github.com/MystenLabs/sui/blob/5a9f37431c473fa2f6d49abecbcc6a6d7190f533/external-crates/move/crates/move-bytecode-verifier/README.md).
CellScript emits untyped RISC-V for CKB-VM, so equivalent assurance requires a
verifiable lowering boundary before machine code plus a separate structural
ELF checker. Recovering the complete CellScript type/resource semantics from an
arbitrary ELF is not a credible 0.24 promise.

The same comparison informed the package trust closure. Sui's new package
design records complete dependency graphs, manifest digests,
environment-specific resolution, and explicit repinning. CellScript adopts
those resolution principles in `Cell.lock` v3, adapted to CKB genesis identity
and immutable Registry snapshots, without importing Move/Sui package or object
identity. See the pinned upstream
[package design](https://github.com/MystenLabs/sui/blob/5a9f37431c473fa2f6d49abecbcc6a6d7190f533/external-crates/move/crates/move-package-alt/design/DESIGN.md).

## Release Principles

1. **Generation and admission are different authorities.** The compiler emits
   artifacts and evidence; a smaller checker decides whether the declared
   artifact contract is internally valid.
2. **The checker is bounded.** Every module, function, basic block, edge,
   instruction, source-map record, and proof record has an explicit count or
   byte limit before traversal.
3. **Machine-code claims stay structural unless independently replayed.** An
   ELF checker may prove section, instruction, CFG, frame, ABI, and syscall
   invariants. It must not claim full source equivalence merely because hashes
   and metadata agree.
4. **Fast tests and authoritative tests are labelled separately.** Simulator
   success is development evidence; CKB-VM execution is runtime evidence; live
   RPC acceptance and commitment remain chain evidence.
5. **No new source edition.** Edition 2026 remains the sole source-semantics
   epoch. A metadata or artifact-contract schema may advance independently once
   its exact shape is frozen.
6. **Runtime adapters do not become hidden language semantics.** Fiber, RGB++,
   and Myelin continue to consume explicit compiler, artifact, deployment, and
   chain evidence through separate adapters.

## Pillar 1: Verified Artifact Boundary

### 1.1 Stable Verified Lowering Record

Define one canonical, versioned lowering record emitted after typed semantic
analysis and before final assembly layout. It is an audit artifact, not a
second executable format.

The first version records only facts that a small checker can validate:

- module, compiler, edition, resolved-profile, source, and artifact identities;
- action, lock, and reachable helper entry identities;
- typed function signatures and fixed-width storage classes used by lowering;
- basic-block identifiers, terminators, typed edges, and call edges;
- frame size, stack-slot kind/width/alignment, outgoing argument area, and
  declared scratch-register avoid sets;
- effect/capability summaries and the exact ProofPlan obligations assigned to
  each entry/block;
- CKB syscall contracts, source/index domains, return-code checks, and bounded
  buffers used by each call site;
- runtime-error exits and their stable error codes;
- final machine-code range and digest for each mapped block after assembly
  layout.

The record must use canonical serialization and a domain-separated hash. It
must not include compiler-internal pointers, nondeterministic map order,
absolute build paths, or opaque prose as an enforcement field.

### 1.2 Independent Checker Crate

Add a standalone checker crate with a deliberately narrow dependency graph.
It must not call the parser, resolver, type checker, optimizer, normal lowering
pipeline, or code generator. Shared types are limited to a versioned schema,
stable diagnostics, canonical hashing, and minimal ELF/Molecule utilities.

The checker validates:

- record schema, canonical order, referential integrity, uniqueness, and
  declared limits;
- CFG entry/exit shape, terminators, branch targets, call targets, recursion
  policy, unreachable-block policy, and frame/call ABI consistency;
- stack-slot width/alignment, outgoing stack arguments, fixed-byte storage, and
  scratch-register declarations;
- effect/capability and ProofPlan coverage consistency at the stable lowering
  boundary;
- ELF class, architecture, sections, entry, text/rodata bounds, prohibited
  dynamic/linker state, and artifact identity;
- the emitted RISC-V instruction allowlist, aligned instruction decoding,
  mapped branch/call targets, stack-pointer deltas, return paths, and declared
  syscall sites; and
- agreement among source-map ranges, lowering-block digests, artifact bytes,
  compile metadata, receipt, and resolved compatibility profile.

The first checker is not required to prove arbitrary instruction-level
equivalence between the typed IR and RISC-V. Any unproven relationship remains
named `binding-verified` or `structurally-verified`, never
`semantically-equivalent`.

### 1.3 Metering And Failure Contract

Checker budgets are inputs to the compatibility profile or admission policy,
not ambient host limits. At minimum, enforce limits for artifact bytes,
record bytes, functions, blocks, edges, instructions, call depth, stack-frame
bytes, proof records, source-map intervals, and diagnostic output.

Budget exhaustion returns one stable rejection code. Invalid input must never
panic, recurse without a checked bound, allocate from attacker-controlled
counts before validation, or emit unbounded diagnostics.

### 1.4 Mutation, Property, And Corpus Evidence

Maintain three independent evidence sets:

- valid compiler-produced artifacts that must pass;
- deterministic mutations of sections, instructions, branches, frames, call
  sites, hashes, proof links, and source maps that must fail with the expected
  checker code; and
- parser/checker fuzz inputs whose minimum requirement is bounded execution and
  no panic.

At least one mutation must target every enforced invariant. A test that merely
changes a hash does not cover CFG, ABI, stack, syscall, or ProofPlan checking.

### Acceptance Boundary

- Every production example ELF and Registry verifier fixture passes the
  standalone checker.
- Every seeded invalid mutation is rejected with its expected stable code.
- The checker has no dependency on compiler front-end or codegen crates.
- Re-running the checker on the same input produces byte-identical JSON.
- Budget exhaustion and malformed length/count fields are negative tests.
- `cellc verify-artifact` reports binding verification, structural
  verification, lowering-record verification, CKB-VM evidence, and chain
  evidence as separate fields.
- The Registry worker can execute the checker in a least-privilege process
  without loading the compiler for artifact-only admission.

## Pillar 2: Executable Package Tests And Source Maps

### 2.1 `cellc test` Executes

Keep the existing package test discovery and expectation conventions where
possible, but make success mean that a selected execution backend actually ran.

The initial backends are:

- `simulator`: deterministic, fast, explicitly non-consensus execution for
  development feedback; and
- `ckb-vm`: compiled ELF execution through the maintained CKB test boundary,
  used for authoritative runtime acceptance.

Test output always records backend, compiler/artifact/checker identities,
profile, entry, inputs, result, runtime error, cycles when available, and the
evidence tier. A package cannot label simulator-only success as CKB-VM evidence.

### 2.2 Versioned Scenario Contract

Define a small, versioned scenario format for transaction-shaped tests. It may
be TOML or JSON after an implementation spike, but one canonical format must be
chosen before release. It describes:

- input and output Cells, capacities, data, lock/type scripts, and named prior
  outputs;
- CellDeps, header deps, `since`, witnesses, lock args, and canonical
  `WitnessArgs.input_type` entry data;
- the action or lock entry under test and its typed parameters;
- positive acceptance or one exact `CellScriptRuntimeError` expectation;
- multi-step Cell replacement with explicit consumed/live state; and
- cycle, transaction-size, and occupied-capacity limits when the backend can
  measure them.

Unknown fields, ambiguous indexes, duplicate names, stale references, missing
Cells, unsupported evidence requests, and mismatched target profiles fail
before execution.

### 2.3 Semantic Coverage

Coverage is tied to compiler evidence rather than only source lines. Reports
include:

- action and lock entries;
- source branches and lowering blocks;
- ProofPlan obligations and evidence tiers;
- runtime-error paths;
- CKB syscall sites; and
- positive/negative transition edges for declared flows.

Coverage never claims that an unexecuted branch is safe. It only says which
declared contract surfaces were exercised by which backend and fixture.

### 2.4 Source-To-Artifact Map

Emit a canonical source map from source spans through typed/lowering blocks to
assembly/ELF instruction ranges. The map must survive deterministic rebuilds,
exclude absolute paths, reject overlapping or out-of-range records, and bind to
the artifact and lowering-record hashes.

Extend existing inspect/trace surfaces rather than creating unrelated tools:

- source-linked artifact inspection;
- source-linked CKB-VM trace rows;
- source-linked checker diagnostics; and
- coverage views keyed by action, lock, ProofPlan obligation, and runtime error.

### Acceptance Boundary

- A package test cannot pass without naming and running a backend.
- Positive and negative fixtures execute under `ckb-vm`; expected failures
  match exact stable runtime codes.
- Multi-step scenarios prove consumed inputs become dead and declared outputs
  become the next step's live inputs in the local harness.
- Source maps round-trip every mapped instruction range and reject overlap,
  gaps that claim coverage, path escape, and artifact mismatch.
- Coverage reports distinguish simulator, CKB-VM, and chain evidence.
- Existing stateful release scenarios remain the oracle and are reused or
  imported; the package runner does not fork their CKB semantics.

## Pillar 3: Myelin Adapter Re-Convergence

### Scope Decision

Do not add an `off-chain-session`, `myelin`, or `myelin_extended` CellScript
target profile in 0.24.

Myelin's current architecture already removes the 0.23 roadmap's original
reason for such a profile:

- CellScript is not vendored into the Myelin workspace;
- Myelin calls an independently versioned compiler process through a lock and
  binary/source/artifact/metadata attestation boundary;
- production compiler requests use the `ckb` target profile;
- session and court execution force Myelin's `CkbStrict` VM semantics; and
- Myelin-only scheduler/finality/DA commitments remain explicit sidecar
  evidence rather than CKB transaction fields.

Putting `MyelinExtended` into CellScript would blur, not close, that boundary.

### 3.1 Upstream Toolchain Handoff

Coordinate one explicit adapter-lock transition from the reviewed 0.22 patch
line to the completed 0.23 identity set:

- Edition 2026;
- current compiler release/revision and Rust toolchain;
- metadata/source/artifact/constraints schema versions;
- resolved compatibility-profile hash;
- canonical `WitnessArgs.input_type` ABI with no raw-witness compatibility;
- compiler executable, source revision, artifact, metadata, lowering record,
  source map, and checker digests; and
- the independent checker version and policy budget.

No fallback reader or alias is added merely to accept the older adapter lock.

### 3.2 Scheduler Evidence Boundary

Continue using CellScript's typed access/scheduler metadata as an untrusted
template. Myelin resolves final conflict hashes from authenticated concrete
Cells and a validated full type-script declaration. Binding names remain
diagnostics, and scheduler plans remain sidecars bound to the raw transaction
identity.

The 0.24 checker validates only that the compiler's access template is
internally well-formed and bound to the artifact. It does not claim that a
Myelin conflict key was resolved correctly; that remains Myelin state-layer
evidence.

### Acceptance Boundary

- Myelin contains no vendored CellScript compiler source or workspace member.
- The adapter rejects the old raw-witness compatibility identity and every
  mismatched compiler/checker/source/artifact/metadata digest.
- Court-facing requests compile under `ckb`; `MyelinExtended` never appears in
  a CellScript compatibility profile.
- The deterministic session fixture produces the same state-transition
  commitments under the static committee and Tendermint, with different
  finality evidence.
- Myelin's production gate verifies the exact pinned CellScript/checker pair;
  skipped external workloads remain labelled skipped rather than passed.

## Pillar 4: Conditional Fiber And RGB++ Evidence Promotion

This is a coordinated evidence track, not a reason to weaken the core 0.24 exit
criteria.

### Fiber

- Complete the declared pinned lifecycle and negative matrix using regular,
  non-empty, content-addressed evidence files under an explicit evidence root.
- Bind Fiber binary revision, build provenance, node configuration, restart or
  capability-detected hot-load state, asset deployment identity, transaction
  hashes, and negative outcomes independently.
- Promote `scripts/cellscript_fiber_acceptance.sh` into release mode only after
  the complete reproducible matrix passes from a clean environment.
- Preserve the no-profile compiler rule and the distinct evidence states
  `StaticallyCompatible`, `LocalNodeConfiguredRestartRequired`,
  `LocalNodeAdvertised`, `ChannelReady`, and `TopologyCertified`.

### RGB++

- Keep RGB++ outside `std::*` and package it as an ecosystem adapter.
- Pin RgbppLock, BtcTimeLock, BTC SPV, witness/commitment, deployment, and
  confirmation identities before promotion.
- Require paired CKB and Bitcoin-side fixtures, including reorg/finality
  assumptions and negative cases.
- Do not call hash/Merkle helpers Bitcoin SPV and do not compose a Spore-over-
  RGB++ claim before both adapters independently pass.

### Acceptance Boundary

- Incomplete rows remain pending; representative samples do not close the
  matrix.
- External evidence is content-addressed and path-confined.
- Operator identity, binary reproducibility, configuration, live transaction
  observation, and topology certification remain separate claims.
- Failure to obtain external evidence does not relax or relabel the core
  compiler/checker/test outcomes.

## Package Evolution Closure

0.24 now ships the resolution subset of the package-evolution design:

- `Cell.lock` v3 carries a canonical source DAG with outgoing alias edges,
  dependency-manifest digests, source hashes, and exact source identities;
- build/check/test are lock-authoritative, while lock/update/add/remove/install
  are the explicit repin boundary;
- standard SemVer, local package aliases, features, optional dependencies, and
  test-only roots are resolved into mode-qualified graph nodes;
- CKB environment roots bind `chain_id` plus genesis hash and require explicit
  selection when dependency overrides exist;
- Git branches normalize to full commits, Registry versions to exact snapshot
  URLs and SHA-256 revisions, and frozen/offline builds use only immutable
  caches; and
- bounded SHA-256-pinned external resolvers normalize to ordinary immutable
  sources at update time and never execute during a locked build.

The remaining package-evolution work stays later-release scope: source/API and
action/lock ABI upgrade reports; Cell/Molecule layout, ProofPlan/effect,
builder, Type ID, and CellDep compatibility; visibility-default changes; and
the independent live-state readability/spendability versus authorization/
predicate-security axes. Merely resolving two nodes does not make conflicting
CellScript module/type identities compatible.

## Website And Stable-Release Integrity

The 2026-08-13 production deployment audit found that the 0.24 website branch
had diverged before the two website commits that published the 0.23 stable
release identity. The resulting build contained the 0.24 Registry and
Playground experience, but its homepage still advertised `v0.22.0`, its
Playground still loaded the 0.22 WASM bundle, and its distribution regression
test incorrectly required those stale identities. A green website build was
therefore not sufficient evidence that the public release identity was current.

The corrected website gitlink `00f0e2cb184c1343d2c6b57aa6a413028976a3e0`
closes that gap:

- the homepage release card names the current stable release `v0.23.0`, its
  2026-08-11 publication date, and the exact GitHub release URL;
- the Playground loads the released 0.23 WASM asset identified by
  `20260811-v0.23.0-fa369818` and SHA-256
  `fa369818631532c657e73e970b6138e3a231d532a073d428dfe7f61686135dd5`;
- the homepage and distribution checks reject a stale release link, tag,
  compiler asset version, compiler version, or WASM digest; and
- the production site is built from the parent repository's exact website
  gitlink rather than from whichever branch happens to be checked out in a
  developer's submodule worktree.

Before any later website deployment, the checked-in GitHub activity snapshot
must be regenerated and reviewed against GitHub's published release state, and
the website branch must include the latest stable-release synchronization
before feature work is layered on top. The homepage continues to advertise the
latest stable tag; the 0.24 nightly branch and these development release notes
do not turn into a stable release merely because their website changes are
deployed.

## Gate Integration

### `dev`

- schema/canonicalization tests;
- quick checker pass over representative artifacts;
- quick invalid-mutation corpus;
- simulator package tests;
- source-map structural checks; and
- `git diff --check` plus existing native source policy.

### `ci`

- all standalone checker tests and clippy;
- complete deterministic invalid-mutation/property corpus;
- package simulator and CKB-VM tests;
- source-map round-trip and semantic coverage fixtures;
- Registry worker/checker integration; and
- current website/WASM/package checks, including the stable release tag,
  compiler asset identity, and exact WASM digest.

### `backend`

- full lowering-record validation over all generated backend surfaces;
- source-map-to-ELF range validation;
- instruction/CFG/frame/ABI/syscall checks;
- full backend mutation corpus; and
- existing stateful CKB scenarios.

### `release`

- all production artifacts rebuilt cleanly and accepted by the standalone
  checker;
- Registry admission evidence names the exact checker and policy;
- authoritative package scenarios are CKB-VM executed;
- production acceptance remains builder- and chain-evidence backed; and
- conditional Fiber/RGB++ evidence is promoted only when its separate matrix
  is complete.

## Sequencing

1. Freeze the threat model, trust states, schema ownership, and checker budgets.
2. Emit deterministic source maps and the minimum stable lowering record.
3. Implement the standalone record/ELF checker and stable diagnostics.
4. Build mutation/property/fuzz evidence and integrate the checker into gates.
5. Turn `cellc test` into an executable simulator/CKB-VM runner with exact
   failure expectations and semantic coverage.
6. Integrate the checker with Registry artifact admission.
7. Coordinate the Myelin adapter-lock handoff to the completed 0.23 identities
   and then to the 0.24 checker contract.
8. Promote Fiber/RGB++ only if their external evidence independently closes.
9. Land the lock-authoritative package graph and versioned Registry profile
   catalog without expanding the source edition or artifact resolver boundary.

Source-map and record schemas land before checker or debugger UX so later
surfaces consume one contract. Myelin handoff follows checker stabilization;
it must not force compatibility aliases into the compiler.

## Risk Register

- **Checker duplicates the compiler**. A second front end would share the same
  bugs and explode the trusted codebase. Mitigation: validate a deliberately
  smaller stable lowering contract and structural ELF properties only.
- **Certificate theatre**. Hashing compiler-authored JSON can look like proof
  without adding an independent check. Mitigation: every promoted claim names
  the independently recomputed invariant and has a matching negative mutation.
- **Verifier denial of service**. Malformed counts or graphs can exhaust the
  worker. Mitigation: validate lengths before allocation and meter every scope.
- **Source-map drift**. Optimizer/layout changes can silently detach diagnostics
  from code. Mitigation: canonical post-layout ranges, non-overlap checks,
  block-byte digests, and rebuild tests.
- **Simulator mistaken for consensus**. Fast tests may be overclaimed.
  Mitigation: mandatory backend/evidence-tier fields and CKB-VM promotion for
  authoritative cases.
- **Myelin semantics leak into CKB**. A convenience profile could make
  off-chain extensions look court-compatible. Mitigation: keep the compiler
  target `ckb`; record Myelin semantics and projection receipts in Myelin.
- **External matrices block core progress**. Fiber/RGB++ depend on external
  binaries, networks, and operators. Mitigation: preserve independent pending
  states and never lower the core checker/test exit criteria.
- **Package extensibility expands the build TCB**. Mutable branch lookup,
  plugin execution, and broad artifact coercion could make builds
  non-reproducible. Mitigation: explicit repinning, exact cached sources,
  bounded hash-pinned update-time resolvers, and a fail-closed profile catalog;
  visibility, semantic upgrade policy, and transaction composition remain out
  of scope.

## Non-Goals

- No Move bytecode, Move VM, Sui object model, UID, shared-object consensus,
  dynamic fields, or `TxContext` surface.
- No verifier for arbitrary RISC-V programs.
- No claim of complete source-to-ELF semantic equivalence in the first checker.
- No new CellScript edition or annual edition cadence.
- No general threading, actor, channel, or session-type syntax.
- No `MyelinExtended` CellScript target profile.
- No Fiber-specific compiler profile or name-matched structural widening.
- No claim that local CKB-VM evidence is mainnet deployment or commitment.
- No visibility-default break, implicit environment selection, unrestricted
  resolver plugins, automatic semantic upgrade policy, or claim that
  multi-node resolution makes conflicting module/type identities compatible.
- No formal prover clone as a substitute for executable and independently
  checked evidence.

## Exit Criteria

The 0.24 core is implemented. The checklist distinguishes repository-owned
evidence from the remaining external handoff and promotion checkpoints:

- [x] The verified lowering record and source-map schemas are versioned,
  canonical, documented, hash-bound, and rejected on unknown fields/versions.
- [x] The standalone checker is independent of the compiler front end/codegen,
  bounded, panic-free under its corpus, and emits stable rejection codes.
- [x] The deterministic mutation and malformed-input corpora cover every stable
  rejection class, including CFG reachability and machine-stack declarations;
  compiler-produced ELF fixtures pass. Full production-example acceptance is
  retained by the release gate.
- [x] `verify-artifact` distinguishes binding, structural, lowering-record,
  CKB-VM, and chain evidence.
- [x] `cellc test` executes both named backends, and authoritative negative
  cases match exact runtime errors.
- [x] Multi-step package scenarios and semantic coverage reports pass and bind
  to the exact artifact/checker identities.
- [x] Source-linked checker, trace, and coverage records round-trip to valid ELF
  instruction ranges.
- [x] Registry artifact-only verification uses the standalone checker in a
  bounded worker and records its version/policy.
- [x] `Cell.lock` v3 is manifest-bound and graph-structured; standard SemVer,
  feature/test roots, aliases, exact Git/Registry pins, CKB environments,
  frozen/offline behavior, explicit repinning, and bounded external resolver
  normalization have positive and fail-closed regressions.
- [x] Registry profile admission uses a versioned catalog and only
  `cellscript_source` is dependency-resolving.
- [x] The production website advertises the actual current stable release,
  binds the matching released Playground WASM, and rejects stale release or
  compiler-asset identities in its build regressions.
- [ ] The Myelin adapter pins and verifies the upstream compiler/checker
  contract without vendoring compiler source or accepting raw-witness aliases.
  CellScript publishes and tests the versioned handoff contract; Myelin's exact
  release lock remains pending the final clean CellScript release commit.
- [x] `dev`, `ci`, and `backend` pass for merge readiness; `release` is required
  before any production CKB claim.
- [x] Fiber/RGB++ remain explicitly pending because their complete declared
  external matrices are not present; no sample has been promoted or relabelled.

## Roadmap Discipline

- Completed work points to tests, reports, or release notes.
- Deferred work names the missing authority, evidence, or design decision.
- A generated certificate is not called independently verified until a smaller
  checker recomputes the claimed invariant.
- Simulator, CKB-VM, RPC admission, commitment, and confirmation remain
  separate evidence tiers.
- CKB source, Script, transaction, syscall, RPC, and deployment claims are
  checked against official CKB sources rather than memory.
- No feature is called implemented until compiler, metadata, CLI, LSP/editor,
  tests, examples, docs, and the matching gate agree on the same boundary.
