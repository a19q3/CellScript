# CellScript and Sui Move Language Completeness Gap Analysis

**Status**: accepted design input for `nightly-0.25`; completion is tracked in
the [0.25 implementation roadmap](CELLSCRIPT_0_25_ROADMAP.md)

**Analysis date**: 2026-08-12

**CellScript baseline**: `nightly-0.24` at
`89a4206939f97fb7ec1452ea6209408ef6d50c8d`

**Sui Move baseline**: MystenLabs `sui` at
`5a9f37431c473fa2f6d49abecbcc6a6d7190f533`

## Purpose

This document answers a narrower question than "should CellScript copy Move?":

> What is still missing before CellScript can be treated as a complete,
> composable, CKB-native smart-contract language rather than a capable
> contract DSL with a deliberately bounded surface?

Sui Move is the comparison baseline because it combines a mature resource
language, a typed bytecode and verifier boundary, a module/package system, a
large toolchain, and a production chain integration. It is not the target
runtime or object model for CellScript.

The comparison therefore separates:

1. generally useful language and assurance capabilities that CellScript still
   needs;
2. capabilities for which CellScript should use a CKB-native design instead of
   Move's design; and
3. Sui-specific features that should not be copied.

This is a source-and-tooling comparison. It does not claim formal equivalence,
security equivalence, ecosystem equivalence, or equal production history.

## Executive Conclusion

CellScript 0.24 is already strong in areas that a generic language comparison
can easily miss:

- CKB Cell inputs, outputs, locks, type identity, witnesses, capacity, `since`,
  CellDeps, and source views are first-class or evidence-visible;
- actions, flows, explicit Cell effects, scoped invariants, ProofPlan records,
  builder assumptions, and stable runtime errors make transaction policy
  auditable;
- Edition 2026, resolved compatibility profiles, `Cell.lock` v3, immutable
  dependency identities, Registry admission, and generated builders form a
  substantial distribution boundary;
- the compiler emits final RISC-V ELF, canonical lowering and source-map
  sidecars, and a compiler-independent structural checker; and
- package scenarios execute under both a simulator and CKB-VM, with separate
  evidence labels.

The remaining distance is not mainly more CKB syntax. It is five structural
closures:

1. **a general but CKB-bounded parameterized type system**;
2. **a composable value ownership and borrowing model**;
3. **a stable module, visibility, public API, ABI, and upgrade-compatibility
   contract**;
4. **an independently checkable typed semantic boundary below the compiler**;
   and
5. **runtime and standard-library closure for every source construct presented
   as supported**.

There are also important secondary gaps in integers, operators, control flow,
pattern matching, reusable collections, error values, coverage/debugging, and
CKB authorization policy. These matter, but they should not be allowed to hide
the five architectural gaps above. General bitwise operations, bounded runtime
text values, and source-local test declarations are concrete examples: all
would improve real authoring workflows, but none substitutes for the type,
module, runtime, or independent-verification closures.

CellScript should not measure completeness as feature-for-feature parity with
Sui Move. A better completion test is:

> Can ordinary audited CKB contracts be written as reusable packages, checked
> without trusting the whole compiler, upgraded under explicit compatibility
> rules, tested against transaction-shaped execution, and compiled without a
> supported source construct falling into metadata-only or fail-closed runtime
> behavior?

By that test, CellScript is a substantial CKB contract language, but it is not
yet a complete general-purpose resource language.

## Baseline And Method

The CellScript baseline is the 0.24 merge-candidate line described by the
[0.24 release notes](../docs/releases/CELLSCRIPT_0_24_RELEASE_NOTES.md) and
[0.24 roadmap](CELLSCRIPT_0_24_ROADMAP.md). The comparison uses the actual
[CellScript AST](../src/ast/mod.rs),
[type checker](../src/types/mod.rs),
[collections boundary](../docs/CELLSCRIPT_COLLECTIONS_SUPPORT_MATRIX.md), and
[verified artifact boundary](../docs/CELLSCRIPT_VERIFIED_ARTIFACT_BOUNDARY.md),
not planned syntax from historical roadmaps.

The Sui Move baseline is pinned rather than described as an unversioned moving
target. Primary references are:

- the pinned
  [Move language book](https://github.com/MystenLabs/sui/blob/5a9f37431c473fa2f6d49abecbcc6a6d7190f533/external-crates/move/documentation/book/src/SUMMARY.md);
- the compiler's pinned
  [edition feature gates](https://github.com/MystenLabs/sui/blob/5a9f37431c473fa2f6d49abecbcc6a6d7190f533/external-crates/move/crates/move-compiler/src/editions/mod.rs);
- the pinned
  [typed bytecode format](https://github.com/MystenLabs/sui/blob/5a9f37431c473fa2f6d49abecbcc6a6d7190f533/external-crates/move/crates/move-binary-format/src/file_format.rs);
- the pinned
  [Move bytecode verifier](https://github.com/MystenLabs/sui/tree/5a9f37431c473fa2f6d49abecbcc6a6d7190f533/external-crates/move/crates/move-bytecode-verifier/src);
- the pinned
  [package-alt design](https://github.com/MystenLabs/sui/blob/5a9f37431c473fa2f6d49abecbcc6a6d7190f533/external-crates/move/crates/move-package-alt/design/DESIGN.md);
- the pinned
  [Move CLI command surface](https://github.com/MystenLabs/sui/blob/5a9f37431c473fa2f6d49abecbcc6a6d7190f533/external-crates/move/crates/move-cli/src/lib.rs); and
- the pinned
  [Sui-specific verifier passes](https://github.com/MystenLabs/sui/tree/5a9f37431c473fa2f6d49abecbcc6a6d7190f533/sui-execution/v3/sui-verifier/src).

Status terms in the tables mean:

| Status | Meaning |
|---|---|
| Strong | CellScript has a deliberate, tested CKB-native answer. |
| Partial | A useful subset exists, but ordinary composition still reaches a documented boundary. |
| Missing | There is no general source-level contract for the capability. |
| Different | Move's feature solves a chain-model problem that should not be copied literally. |

## Comparative Matrix

### 1. Core Values, Expressions, And Control Flow

| Area | Sui Move baseline | CellScript 0.24 | Status | Required direction |
|---|---|---|---|---|
| Integer types | `u8`, `u16`, `u32`, `u64`, `u128`, `u256` with typed bytecode operations | `u8`, `u16`, `u32`, `i32`, `u64`, `u128`; the lexer stores integer literals as `u64` | Partial | Add arbitrary-width literal parsing, close all declared `u128` operations, and decide whether general `u256` is source-level or a checked fixed-width library type. Keep `i32` because it serves CKB-relative-distance use cases. |
| Arithmetic and operators | Arithmetic, comparison, bitwise operations, shifts, casts, and checked VM behavior over supported integers | Arithmetic, comparison, boolean `&&`/`||`, unary negation/not, `=` and `+=`; no source bitwise or shift operator family | Partial | Add the missing operator families with overflow, division-by-zero, shift-width, constant-folding, IR, codegen, checker, and diagnostic contracts. |
| `u128` execution | General typed bytecode value | Wide local storage plus checked add/sub/mul/div and comparison paths, but modulo and some non-addressable or non-materialized shapes fail closed | Partial | A declared primitive must have a complete executable matrix or be rejected statically for the unsupported operation. Runtime traps must not be the ordinary feature-discovery mechanism. |
| Conditionals and loops | Expression conditionals, `while`, `loop`, `for`, labeled blocks, `break`, and `continue` | `if`, `for`, `while`, expression `if`; linear state changes in loops are conservatively rejected | Partial | Add bounded `break`/`continue` and labeled control only after linear-state merge rules and lowering/checker coverage are explicit. An unbounded `loop` is optional for CKB and may remain absent. |
| Failure surface | `abort` and `assert` with abort codes | `require`, `assert`, stable compiler/runtime registries, and fail-closed exits; no general source error value or propagation type | Partial | Keep stable verifier failures, then add a typed error/result design only if it remains cheap, inspectable, and compatible with CKB Script exit semantics. |
| Functions | Typed functions, generics, visibility, entry/native distinctions, method syntax, macro functions, and lambdas in Move 2024 | Typed `fn`, `action`, and `lock` callables with effect classes; no general type parameters, visibility modifiers, higher-order values, lambdas, or source macro functions | Partial | Treat generic functions and API visibility as structural work. Treat lambdas, method syntax, and macros as later ergonomics unless a standard-library design requires them. |
| Constants | Typed constants | Typed constants, but numeric literal width is still rooted in `u64` tokenization | Partial | Reuse the arbitrary-width literal parser and require compile-time representability. |

### 2. Data Types, Generics, And Abilities

| Area | Sui Move baseline | CellScript 0.24 | Status | Required direction |
|---|---|---|---|---|
| Structs | Named and positional structs with type parameters and abilities | Concrete named-field structs with validity blocks and CKB layout metadata | Strong for concrete CKB values; missing generics | Preserve schema visibility while adding parameterized value types. |
| Enums and pattern matching | Generic enums, variants, nested patterns, exhaustive `match`, Move 2024 enum feature | Concrete fixed-width payload enums and a limited variant/wildcard match; generic declarations, nested payload patterns, variable-width and recursive payloads are rejected | Partial | Close nested and named payload patterns, generic enums, exhaustiveness, layout recursion rules, and variable-width serialization policy. Recursive persistent types may remain forbidden. |
| User-defined generics | Struct, enum, and function type parameters with constraints and instantiation checks | No general user type parameters. `Vec<T>`, `BoundedCellSet<T, N>`, and `BoundedList<T, N>` are compiler-recognized bounded forms, not a general parameterized type system | Missing | Introduce explicit type-parameter AST/IR nodes, substitution, inference boundaries, monomorphization identity, metadata-visible instantiations, recursion limits, and deterministic layout rules. |
| Phantom parameters | `phantom T` with verifier-enforced usage rules | Deferred historical design; no source-level phantom parameter | Missing | Add phantom parameters for CKB asset/type tags only together with generic ability and layout rules. A phantom tag must affect type identity without silently affecting serialized layout. |
| Value abilities | `copy`, `drop`, `store`, `key`, propagated through generic instantiation | Closed Cell operation capabilities: `store`, `create`, `consume`, `destroy`, `replace`, `burn`, `relock`, `retarget_type`, `read_ref` | Different and incomplete | Do not rename Cell effects into Move abilities. Add a separate value-ability layer for duplication, discard, storage/layout, and linearity, then keep Cell lifecycle authority as the existing second layer. |
| Generic constraints | Ability constraints on type parameters | No user-declared type constraints | Missing | Define CKB-native constraints such as fixed-width/serializable/non-linear/cell-backed plus explicit lifecycle capability requirements. Constraints must be checker-visible after monomorphization. |
| Type aliases and abstraction | Module-owned types and visibility boundaries provide abstraction | Named types exist, but there is no general public/private type API or opaque representation boundary | Missing | Tie representation visibility and layout/API compatibility to the module system rather than adding purely textual aliases first. |

The most important design point is that CellScript needs **two related but
non-interchangeable algebras**:

1. value properties used by generic typing, such as whether a value can be
   copied, discarded, stored, serialized, or given a fixed layout; and
2. Cell authority used by actions, such as whether a particular Cell type can
   be created, consumed, replaced, burned, relocked, or read by reference.

Move's four abilities cannot directly express the second algebra, while the
current CellScript capability registry cannot safely stand in for the first.

### 3. Ownership, References, And Cell State

| Area | Sui Move baseline | CellScript 0.24 | Status | Required direction |
|---|---|---|---|---|
| Linear values | Absence of `copy`/`drop` and verifier checks make resource use linear | Flow-sensitive `Available`, `Consumed`, `Transferred`, and `Destroyed` states for Cell-backed bindings | Strong for explicit Cell flows | Preserve the explicit state model and make it participate in generic instantiation and module calls. |
| References | First-class immutable and mutable reference types with independent bytecode reference-safety verification | `&T` helper views plus explicit read-only `borrow root as view { ... }` regions; references cannot escape or be stored; `&mut` Cell parameters are rejected | Partial by design | Keep Cell updates as explicit input-to-output replacement. Add either safe local mutable references for non-Cell values or by-value update APIs; in both cases specify path-sensitive aliasing, field borrows, reborrowing, and call boundaries. |
| Borrow checker boundary | Type checker plus bytecode verifier rechecks reference safety | Compiler type checker records borrow regions; the artifact checker validates structural records but does not independently reconstruct source-level borrow safety | Missing independent semantic check | Carry borrow/ownership facts into a typed verified representation that a smaller checker can recompute. |
| Aggregate ownership | Generic structs/enums/vectors propagate abilities | Cell-backed values are rejected in hidden generic vectors; concrete payload enums have explicit ownership/storage rules | Strong safety boundary, partial expressiveness | Keep hidden `Vec<Cell<T>>` ownership rejected. Add explicit bounded Cell collection primitives only with destructuring, membership, discharge, and runtime evidence. |
| State mutation | Object/resource mutation through references under Sui rules | Cell state is modeled as consumed input plus proposed output, `transition`, preservation requirements, and named outputs | Different and strong | This is the correct CKB-native replacement for general object mutation. Improve composability without weakening the replacement model. |

General `&mut Cell<T>` is not a completeness requirement for CellScript. CKB
does not mutate a live Cell in place. What is required is a complete and
composable way to write reusable functions over proposed state transitions,
without forcing every reusable value helper to be manually inlined.

### 4. Modules, Packages, Public APIs, And Upgrades

| Area | Sui Move baseline | CellScript 0.24 | Status | Required direction |
|---|---|---|---|---|
| Module namespace | Address/package-qualified modules and module-owned types/functions | Named modules, `use` imports, package source loading, duplicate identity checks, and compile-time inclusion | Partial | Define canonical package/module/type identities independent of local paths and source aliases. |
| Visibility | Private, `public`, `public(package)`, deprecated friend visibility, entry restrictions, and type visibility work | No source visibility modifier on structs, enums, functions, actions, or locks | Missing | Add an explicit exported API surface with a conservative default. Do not silently change existing visibility under Edition 2026. |
| Separate verification | Compiled dependency modules are typed artifacts verified against signatures and dependencies | Dependencies are source-authenticated and compiled into final entry artifacts; there is no stable separately verified CellScript module artifact | Missing | Define a canonical interface artifact and decide whether implementation units remain source-linked or gain separately verified typed units. |
| Public API compatibility | Function/type/ability compatibility machinery exists in the Move binary format | Package identity and build/deployment evidence are strong, but semantic API/ABI/layout upgrade policy remains deferred | Missing | Add API, ABI, serialized-layout, capability/effect, entry/witness, and generated-builder compatibility reports. |
| Dependency locking | Graph-aware Move package work with environments and published identities | `Cell.lock` v3 is manifest-bound, graph-structured, feature/test/environment aware, and lock-authoritative with immutable Git/Registry materialization | Strong | Keep this CKB-native design. The main remaining work is public API compatibility, publication upgrades, and module identity, not basic dependency resolution. |
| Environment identity | Sui chain IDs, published package IDs, address replacement | Explicit CKB `chain_id` plus genesis hash, no implicit environment selection | Strong and deliberately different | Preserve explicit genesis-bound selection; do not copy implicit Sui mainnet/testnet address behavior. |

Package resolution is one of the areas where CellScript 0.24 is already close
to, and in some auditability respects stricter than, the comparison baseline.
The missing layer begins after resolution: what a dependency publicly promises
and whether an upgrade preserves that promise.

### 5. Execution And Independent Verification

| Area | Sui Move baseline | CellScript 0.24 | Status | Required direction |
|---|---|---|---|---|
| Executable form | Typed Move bytecode executed by Move VM | Native RV64 ELF executed by CKB-VM | Different | Keep CKB-VM and RISC-V as the production target. A second production VM is not required. |
| Typed verifier | Bounds, signatures, abilities, data definitions, dependencies, control flow, locals, references, type safety, stack use, instantiation loops, and limits are independently checked | Standalone checker validates canonical records, ELF shape, emitted instructions, CFG, frames, stack restoration, ABI, syscalls, ProofPlan links, digests, and source ranges | Partial | Add an independently recomputed type/ownership/effect layer. Current structural verification must continue to state `semantic_equivalence_claimed = false`. |
| Compiler trust | Invalid compiler output is rejected at the bytecode boundary before execution | The checker reduces trust in metadata/artifact structure but still accepts compiler-authored semantic summaries that it cannot derive from erased RISC-V alone | Partial | Introduce a stable typed lowering unit, proof-carrying semantic record, or another small representation from which ownership, types, call signatures, effects, and obligation discharge can be recomputed. |
| Runtime metering | VM and verifier metering integrated with protocol configuration | CKB cycles, code size, stack, transaction size, and capacity evidence are surfaced through CKB tooling and gates | Strong for CKB | Continue using CKB cycles rather than importing a Move gas model. Add per-language-operation cost regression budgets where compiler transformations can change cost materially. |
| Native boundary | Versioned VM native functions and chain-specific native validation | Inline generated runtime helpers, CKB syscalls, Spawn/IPC, and pinned verifier packages | Partial | Define one small versioned extension ABI with effect, memory, cycle, failure, and artifact-identity contracts. Do not allow arbitrary helper names to expand the trusted surface. |

The hardest gap is caused by the final RISC-V target: most source types are
erased before execution. The 0.24 lowering record is the right starting point,
but checking that compiler-authored blocks match machine blocks is not the same
as independently checking that source ownership and types were lowered
correctly.

A complete design does not require Move bytecode. It does require one of these
equivalent trust boundaries:

- a canonical typed CellScript IR that is independently verified and then
  translated to RISC-V under a checked lowering contract;
- a proof-carrying lowering record rich enough for a small checker to replay
  typing, ownership, effects, and obligation discharge; or
- a mechanically validated translation validator between typed IR and final
  machine behavior for the supported instruction/runtime subset.

The choice needs a separate RFC and prototype. It should not be hidden inside
incremental additions to the current structural checker.

### 6. Runtime And Standard Library

| Area | Sui Move baseline | CellScript 0.24 | Status | Required direction |
|---|---|---|---|---|
| Generic values | General `vector<T>` plus framework collection and option types | Stack-backed local `Vec<T: FixedWidth>` and targeted schema vectors; no general allocation ABI, generic map/set, source `Option<T>`, or Cell-backed generic collection | Partial | Build generic value libraries after the type kernel. Preserve explicit maximum sizes and reject hidden Cell ownership. |
| Runtime text | Move framework `String` and ASCII values are reusable `vector<u8>`-backed library types | CellScript accepts string expressions and supports UTF-8 `String` at documented schema/ABI boundaries, but does not provide a complete general runtime text API | Partial | Add a bounded UTF-8 value/library contract only after generic value layout is stable. Specify byte length, validation, comparison, slicing, allocation/backing, cycles, and serialization rather than treating text as an unbounded heap value. |
| Serialization | VM type layouts and BCS/framework integration | Explicit fixed layouts, Molecule manifests, typed entry/witness ABI, and targeted dynamic fields | Strong for documented shapes | Generalize layout derivation to parameterized types, publish stable layout identities, and add compatibility checking. |
| Resource lifecycle library | Move abilities and Sui framework object/transfer APIs | Audit-visible lifecycle, receipt, accounting, Cell metadata, CKB protocol, and runtime helpers | Strong but narrow | Expand only when helpers lower to explicit effects and evidence. Avoid name-matched business policy in codegen. |
| Cryptography | Broad Sui native crypto surface | Selected inline helpers, Spawn/IPC verifier composition, and pinned external verifier packages; signature authorization remains intentionally explicit | Partial | Standardize a versioned crypto/verifier package interface and close first-class CKB signer/sighash evidence without hiding witness or script-group policy. |
| Errors and optionals | Abort codes plus generic library ADTs such as options | Stable runtime error registry, but no general generic optional/result type | Partial | Generic enums should unlock `Option<T>` and, if justified, `Result<T, E>` without a special compiler path. |

Library size is not itself a completeness metric. CellScript's standard library
should remain smaller than Sui's because CKB has no Sui object runtime. The
required property is that common safe patterns are reusable without compiler
name magic or undocumented fail-closed paths.

### 7. Testing, Diagnostics, And Developer Tooling

| Area | Sui Move baseline | CellScript 0.24 | Status | Required direction |
|---|---|---|---|---|
| Unit and scenario tests | Move unit-test runtime, Sui test scenario support, tracing, instruction bounds | Compile tests plus versioned simulator/CKB-VM scenarios, exact negative errors, and separate stateful CKB acceptance | Strong for current contract model | Extend CKB-VM scenarios so transaction Cells can populate actual syscalls, reducing dependence on a separate oracle for transaction-shaped behavior. |
| Source-local tests | Test and expected-failure declarations can live with Move source | CellScript executable expectations live in compile fixtures and external versioned scenario files rather than a stable source attribute surface | Partial developer experience | Consider `#[test]` and exact expected-failure sugar only as a frontend over the same scenario/evidence model. Source-local tests must not create a second simulator-only definition of execution. |
| Coverage | Source and bytecode coverage, LCOV, differential coverage | Source-linked semantic coverage over scenario/checker records | Partial | Add branch, action, lock, obligation, runtime-error, and final instruction coverage with explicit conservative semantics. |
| Bytecode/artifact inspection | Disassembler, bytecode viewer, decompiler infrastructure | ASM output, metadata, lowering/source maps, trace and audit reports; no first-class ELF disassembly/decompilation workflow | Partial | Add a source-linked artifact inspector and deterministic disassembly command. Decompilation is optional and must not imply source recovery. |
| IDE | Move analyzer and compiler IDE information | LSP diagnostics, completion, hover, definitions, references, formatting, code actions, VS Code, Playground, MCP | Strong | Extend every new type/module feature through the same frontend closure; avoid an IDE-only shadow type system. |
| Formatting, lint, migration | Formatting, compiler lint, edition migration | AST formatter, syntax/role diagnostics, Edition 2026 policy; no edition migration is yet needed | Partial | Add explicit lint levels and API/edition migrations when the first semantic transition is actually designed. |
| Robustness testing | Verifier fuzzers, property tests, transactional compiler/verifier suites | Syntax-combination audit, mutation corpora, backend audits, compiler tests, CKB differential/stateful fixtures | Strong but uneven | Add parser/type/lowering fuzz targets and generic-instantiation/property corpora before expanding the type system. |

### 8. Chain Model And Authorization

| Area | Sui Move baseline | CellScript 0.24 | Status | Required direction |
|---|---|---|---|---|
| Persistent state | Sui owned/shared objects and object runtime | CKB live Cells consumed and replaced by transactions | Different and strong | Keep Cells, Lock Scripts, Type Scripts, CellDeps, and witnesses literal. |
| Identity | `UID`, `key`, object IDs, package IDs | TYPE_ID, script args, field identity, singleton type identity, script hashes, OutPoints, deployment and CellDep evidence | Different and strong | Continue making identity policy explicit per Cell type and transition. |
| Authorization | Sui transaction/object ownership rules and entry-point verifier restrictions | Protected inputs, lock groups, witness/lock-args sources, explicit verifier helpers, builder assumptions; no implicit signer value | Partial | Close explicit sighash/digest mode, script-group scope, replay assumptions, signer evidence, and generated-builder binding before adding signer sugar. |
| Time and capacity | Sui protocol/gas/object rules | CKB `since`, headers, occupied capacity, fee/capacity builder evidence | Strong but partly metadata/builder based | Promote common continuity, capacity, and time policies to source declarations only where generated checks and builder evidence can agree. |
| Transaction composition | Programmable transaction blocks and Sui transaction context | Per-action builders, action metadata, and derived protocol graphs; typed multi-action composition remains deferred | Partial | Add typed composition over Cell footprints, dependency identities, witness slots, capacity/fee constraints, and failure atomicity without importing PTB object semantics. |

## Structural Gaps That Block A "Complete Language" Claim

### P0-A: Parameterized Type And Value-Ability Kernel

Required minimum:

- user-defined type parameters on structs, enums, and functions;
- explicit constraints for value copying/discard, fixed layout, serialization,
  non-linearity, and Cell-backed values;
- phantom parameters with type-identity and layout rules;
- deterministic monomorphization identity and recursion/size budgets;
- metadata and `cellc explain generics` coverage for every instantiation;
- generic type inference that never guesses Cell ownership or capability
  authority; and
- parser, formatter, resolver, type checker, IR, codegen, checker, LSP,
  docgen, package API, tests, and docs closure.

Start with non-Cell fixed-width value generics. Do not begin with generic
Cell-backed collections.

### P0-B: Composable Ownership And Borrowing

Required minimum:

- generic calls preserve linear and Cell capability facts across module
  boundaries;
- read-only borrows remain non-escaping and path-sensitive;
- field/path borrowing and reborrowing have a written alias model;
- local value updates have either safe non-Cell mutable references or explicit
  by-value update functions;
- Cell updates remain input/output replacement, never in-place mutation; and
- the independent semantic boundary can recompute the relevant rules.

### P0-C: Module, Visibility, API, ABI, And Upgrade Contracts

Required minimum:

- canonical module and type identities;
- explicit exported versus package-private source items;
- interface artifacts containing types, generic constraints, effects,
  capabilities, layouts, actions/locks, and entry ABI;
- compatibility modes for source API, serialized layout, runtime ABI,
  capability/effect changes, generated builders, and deployment identity;
- package publication and upgrade reports bound to exact old and new
  interfaces; and
- an Edition 2026 migration plan before changing existing visibility defaults.

### P0-D: Independently Checked Typed Semantics

Required minimum:

- one versioned representation below the frontend that retains types,
  ownership, effects, calls, layouts, and ProofPlan discharge;
- a checker whose production dependency graph excludes parser, resolver,
  optimizer, and code generator;
- bounded checks for substitution, abilities, locals, references/borrows,
  control-flow joins, call signatures, resource discharge, layout, and runtime
  helper contracts;
- a validated link from the typed representation to the existing final ELF
  structural record; and
- mutation, differential, property, and malformed-input evidence for every
  rejection class.

This is an assurance-completeness requirement, not a request to replace CKB-VM
with Move VM.

### P0-E: Supported-Surface Runtime Closure

Required minimum:

- a generated support matrix for every AST/IR operation and type shape;
- compile-time rejection when a selected production target lacks lowering;
- no ordinary supported program discovering missing semantics only through a
  fail-closed runtime path;
- no production claim for a `gap:metadata-only` or
  `gap:runtime-helper-required` obligation;
- full `u128`, enum, collection, cast, output verification, and helper-shape
  matrices; and
- the same boundary in simulator, CKB-VM, metadata, checker, LSP, docs, and
  release gates.

Fail-closed runtime paths remain valuable defense in depth. They should not be
the public-language feature boundary.

### P0-F: CKB Authorization And Transaction Policy Closure

This gap is not exposed by copying Move syntax, but it blocks a complete CKB
contract experience:

- explicit signature/digest primitives with script-group and witness scope;
- signer evidence whose derivation and replay assumptions are inspectable;
- source-visible capacity, time/header, and continuity policies for common
  cases;
- typed multi-action composition over Cell footprints and builder obligations;
  and
- execution evidence binding generated transactions to the exact artifact,
  deployment, CellDeps, witness ABI, and source package.

## Secondary Language And Tooling Gaps

These are important after the P0 design boundaries are fixed:

| Priority | Gap | Completion condition |
|---|---|---|
| P1 | Arbitrary-width integer literals and complete `u128` | Every operator/cast has constant, local, field, parameter, return, call, and negative CKB-VM coverage. |
| P1 | `u256` or equivalent checked wide-value library | CKB protocols can express full-width arithmetic without ad hoc compiler helpers; overflow and cost are explicit. |
| P1 | Bitwise and shift operators | `&`, `|`, `^`, `<<`, and `>>` have typed width/signedness rules, constant folding, overflow/shift-bound diagnostics, IR/codegen/checker coverage, and CKB-VM negative tests. |
| P1 | Complete enum and value patterns | Nested/named payload, struct, tuple, and or-patterns where adopted; exhaustiveness, ownership across arms, layouts, and checker coverage agree. Reference patterns should wait for the borrowing model. |
| P1 | Reusable value collections and `Option<T>` | Implemented through the generic kernel with bounded storage and no hidden Cell ownership. |
| P1 | Bounded runtime text | UTF-8 validation, byte-oriented interop, comparison, bounded construction/slicing, Molecule layout, cycle limits, and failure behavior are explicit. Rich text processing remains off chain. |
| P1 | Public API and upgrade CLI | Package build/publish can emit and compare canonical interfaces before Registry admission. |
| P1 | Transaction-shaped `cellc test` | Scenario Cells, deps, headers, since, and witnesses populate the actual CKB syscalls used by the artifact. |
| P1 | Coverage and artifact inspection | Source, typed IR, final instruction, action/lock, obligation, and error coverage can be inspected together. |
| P1 | Fuzz/property infrastructure | Parser, type substitution, borrow checking, verifier, lowering, and compatibility readers have bounded automated corpora. |
| P2 | `break`, `continue`, and labels | Linear merges and final CFG/source maps remain deterministic and independently checked. |
| P2 | Method and index syntax | Pure desugaring with no new authority, lookup ambiguity, or hidden allocation. |
| P2 | Source-local test sugar | `#[test]` and exact expected-failure declarations compile into the canonical package scenario/evidence model and run under explicitly selected backends. |
| P2 | Lambdas and higher-order helpers | Effects, captures, linear values, monomorphization, and borrow escape are all explicit. |
| P2 | Source macro functions | Expansion is typed, source-mapped, metadata-visible, bounded, and unable to hide Cell effects. |
| P2 | Richer lint and migration UX | Rules are stable, suppressible by explicit identity, and share CLI/LSP diagnostics. |

## Sui Move Features CellScript Should Not Copy Literally

The following are not CellScript completeness gaps:

- Sui `UID` as the universal persistent identity;
- the Sui meaning of Move's `key` ability;
- owned/shared-object consensus and Sui object wrapping;
- dynamic fields backed by the Sui object runtime;
- global Move storage operations—Sui itself rejects them in favor of its object
  model, while CellScript must use CKB transaction views;
- `TxContext`, programmable transaction block object semantics, gas coins, and
  Sui event storage;
- implicit mainnet/testnet environments or published package address
  substitution;
- Move bytecode or a second production VM; and
- unbounded heap collections that obscure CKB Cell cardinality, capacity, or
  cycle limits.

CKB-native counterparts are Cells, Lock and Type Scripts, OutPoints and TYPE_ID,
CellDeps, WitnessArgs, script groups, headers/`since`, capacity, generated
transactions, and CKB-VM evidence.

## Proposed Work Sequence

The sequence below is assigned to the `nightly-0.25` implementation line. The
stage order is normative: a later ergonomic surface does not count as complete
until the earlier semantic and verification boundaries it depends on pass.

### Stage 1: Make The Existing Boundary Mechanically Exact

- Generate a source-type/operation support matrix from compiler registries.
- Convert unsupported production shapes from runtime discovery to compile-time
  diagnostics.
- Close remaining `u128`, enum, collection, output verification, and cast
  holes for the current non-generic language.
- Add parser/type/lowering fuzz and property harnesses before increasing
  expressiveness.

**Exit**: every documented production construct has complete lowering and
CKB-VM coverage, or is rejected before artifact generation.

### Stage 2: Introduce The Parameterized Value Kernel

- Add explicit type parameters, substitution, constraints, phantom parameters,
  and deterministic monomorphization.
- Separate value abilities from Cell lifecycle capabilities.
- Generalize structs, enums, functions, `Option<T>`, and fixed-width value
  vectors first.
- Keep Cell-backed generic collections rejected.

**Exit**: independently generated instantiation metadata agrees across compiler,
checker, LSP, docs, and final layouts for a broad positive/negative matrix.

### Stage 3: Stabilize Modules And Ownership Across APIs

- Add visibility and canonical public interface artifacts.
- Specify local mutation, borrow paths, reborrowing, and generic call behavior.
- Add API/ABI/layout/effect/capability compatibility reports.
- Bind Registry publication and generated builders to interface identities.

**Exit**: a dependency can be upgraded only after a deterministic report shows
which source, layout, runtime, builder, or deployment contracts changed.

### Stage 4: Add The Typed Independent Verifier Boundary

- Choose typed IR, proof-carrying lowering, or translation validation through a
  bounded prototype.
- Recompute typing, ownership, calls, effects, layouts, and obligation discharge
  without compiler frontend/codegen dependencies.
- Link that result to the existing ELF structural checker.

**Exit**: compiler mutations that preserve superficial metadata but violate
typed semantics are independently rejected with stable codes.

### Stage 5: Complete CKB Libraries, Authorization, And Tooling

- Build generic value libraries and a bounded verifier-package ABI.
- Close explicit sighash/signer, capacity, time, continuity, and composition
  contracts.
- Expand transaction-shaped tests, coverage, artifact inspection, debugging,
  fuzzing, and package migration tools.

**Exit**: representative fungible token, NFT, DAO, covenant, AMM, order,
multisig/authorization, and cross-Script composition packages can be implemented
without ad hoc compiler name matching or undocumented external glue, while
their final artifacts remain independently and transactionally checkable.

## Completion Gates

A future "language complete" claim should require all of the following rather
than a checklist of syntax tokens:

| Gate | Required evidence |
|---|---|
| Surface closure | Parser, formatter, resolver, type checker, IR, metadata, codegen, checker, LSP, docgen, examples, and syntax-combination tests agree. |
| Generic safety | Substitution, phantom usage, ability constraints, monomorphization identity, recursion/size budgets, layouts, and negative ownership cases pass. |
| Module stability | Canonical interface emission plus API, ABI, layout, effect, capability, builder, and deployment compatibility reports pass. |
| Runtime closure | Production builds contain no unsupported operation shape and no unresolved fail-closed feature debt. |
| Semantic verification | A compiler-independent checker recomputes typed ownership/effect semantics and binds them to final ELF structure. |
| CKB execution | Positive and exact-negative transaction-shaped CKB-VM scenarios cover actual syscalls, Cell replacement, deps, headers, since, witnesses, capacity, and cycles. |
| Authorization | Signer/digest/script-group/witness and replay assumptions are explicit and builder-bound. |
| Distribution | Exact source, interface, dependency graph, artifact, checker policy, deployment, and Registry identities round-trip. |
| Robustness | Mutation, differential, property, fuzz, malformed-input, resource-budget, and backwards-compatibility corpora pass. |
| Claim discipline | Simulator, structural checker, CKB-VM, devnet, deployment, commitment, and mainnet evidence remain distinct. |

## Decision Summary

CellScript should continue as a CKB-native language, not become Move-on-RISC-V.
The best parts to learn from Move are:

- a real parameterized resource-aware type system;
- independently verified typed execution contracts;
- explicit module and public API boundaries;
- compatibility checking for published code;
- systematic verifier and compiler robustness testing; and
- a coherent standard toolchain around build, test, coverage, inspection, and
  migration.

The parts CellScript should preserve as its own are:

- explicit Cell replacement rather than object mutation;
- separate Lock and Type Script authority;
- transaction-view, witness, CellDep, capacity, and `since` visibility;
- action/flow/ProofPlan and builder-evidence models;
- immutable, genesis-bound package and deployment identities; and
- final CKB-VM RISC-V artifacts with independently labeled evidence.

The shortest credible path is therefore not to add Move 2024 syntax first. It
is to close the current supported runtime surface, build constrained generics
and value abilities, stabilize modules and compatibility, and then extend the
independent checker from structural facts to typed semantic facts.
