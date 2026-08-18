# CellScript 0.25 Language Completeness Roadmap

**Status**: active implementation on `nightly-0.25`

**Started**: 2026-08-12

**Baseline**: `nightly-0.24` at
`89a4206939f97fb7ec1452ea6209408ef6d50c8d`

## Goal

0.25 implements the accepted work from the
[CellScript and Sui Move language completeness gap analysis](CELLSCRIPT_MOVE_LANGUAGE_COMPLETENESS_GAP_ANALYSIS.md).
The target is not Move syntax parity. The target is a complete, composable,
CKB-native language boundary whose public constructs lower completely, whose
package interfaces can be upgraded under explicit rules, and whose typed
semantics can be checked independently of the compiler that emitted the final
RISC-V artifact.

No row in this roadmap is complete because its parser syntax exists. Completion
requires the parser, formatter, resolver, type checker, ownership/flow checker,
IR, metadata, codegen, independent checker, LSP, docgen, examples, negative
tests, and the applicable CKB-VM evidence to agree.

## Release Rules

- Metadata-only compilation may describe a reserved or unsupported construct,
  but an executable production compile must reject that construct before
  artifact generation.
- Cell lifecycle capabilities and generic value abilities remain separate
  algebras. Generic substitution must not grant Cell authority.
- Cell-backed values cannot be hidden in ordinary generic containers.
- CKB state change remains input consumption plus output creation/replacement;
  0.25 does not introduce in-place Cell mutation.
- The independent checker must continue to report
  `semantic_equivalence_claimed = false` until it recomputes the complete typed
  semantic record and binds it to the final ELF.
- Sui object, global-storage, gas-coin, PTB, `TxContext`, and Move VM semantics
  are out of scope.

## Implementation Matrix

Status values are `pending`, `in-progress`, `implemented`, and `verified`.
Only `verified` is release-complete.

| ID | Requirement | Status | Required release evidence |
|---|---|---|---|
| LC-01 | Generated supported-surface matrix | implemented | Compiler-owned operation/type registry, generated Markdown/JSON, freshness gate, and negative drift test. Final full-gate verification remains. |
| LC-02 | Pre-codegen executable-surface rejection | in-progress | `--production` and `--deny-fail-closed` reject with stable diagnostics before ASM/ELF generation; metadata-only Playground analysis remains available. |
| LC-03 | Current non-generic runtime closure | in-progress | Complete `u128`, enum, collection, output-verification, field/index/length/type-hash, lifecycle, and cast matrices with exact-negative CKB-VM tests. `u128` division/remainder now share a complete wide restoring-division path; scalar division/remainder now reject zero with stable runtime code 20. |
| LC-04 | Arbitrary-width integer literals | implemented | Decimal tokens preserve the full supported `u128` range; inference selects `u64` or `u128`; narrower contexts use checked representability diagnostics; constants, IR, formatter round trips, codegen byte materialization, and overflow boundaries are tested. Final full-gate verification remains. |
| LC-05 | Bitwise and shift operations | implemented | `&`, `|`, `^`, `<<`, and `>>` have precedence-preserving parse/format support, contextual integer typing, constant folding, scalar and `u128` IR/codegen, internal assembler closure, stable compile/runtime range errors, editor grammar support, and exact CKB-VM width/signedness/cross-limb vectors. Final full-gate verification remains. |
| LC-06 | Parameterized type kernel | implemented | Struct/enum/function parameters, explicit constraints, inference at typed function calls, deterministic pre-IR monomorphization, nesting/count/identity budgets, canonical metadata, and `cellc explain generics`. Full checker/gate verification remains. |
| LC-07 | Value abilities and phantom parameters | implemented | Closed `copy`/`drop`/`store`/`fixed`/`serializable`/`non_linear`/`cell` registry is separate from Cell capabilities; phantom layout use is rejected and phantom arguments remain in identity metadata. Full checker/gate verification remains. |
| LC-08 | Generic structs, enums, functions, `Option`, and fixed vectors | implemented | Fixed-width value templates, generic helper calls, built-in `Option<T>`, generic fixed arrays, concrete ABI lowering, CKB-VM execution, and hidden Cell-layout rejection are covered. Cross-module template publication remains under LC-11/LC-13. |
| LC-09 | Complete value patterns | implemented | Recursive enum/tuple/struct patterns, binding-free or-patterns, outer exhaustiveness, linear wildcard rejection, fixed aggregate addressability, syntax audit coverage, and exact CKB-VM execution are implemented. Independent checker and final gate verification remain. |
| LC-10 | Composable local ownership and borrowing | implemented | Existing deterministic branch/loop ownership merges now cover field-path read-only borrows, canonical-root reborrowing, non-Cell dereference, lifecycle-crossing rejection, non-escape rules, and explicit generic Pure/ReadOnly call boundaries. Independent checker and final gate verification remain. |
| LC-11 | Canonical module and public interface identity | implemented | `public`, `public(package)`, and `private`; canonical module/item identities; `cellscript-package-interface-v2`; interface hashes; dependency-facing templates without implementation instances; final gate verification remains. |
| LC-12 | API/ABI/layout/effect compatibility | implemented | `cellc interface` and `interface-diff` compare source API, serialized layout, runtime ABI, effects/capabilities, builders, and deployment contracts; breaking reports use E2501. Final gate verification remains. |
| LC-13 | Registry/interface binding | implemented | Signed publish payloads bind the canonical interface/hash; API admission recomputes identity and rejects incompatible upgrades; the standalone verifier rechecks the binding. Final service/gate verification remains. |
| LC-14 | Independently checked typed semantic record | implemented | `cellscript-typed-semantics-v2` records exact types/constants/operations, CFG, calls/effects, ownership, borrows, layouts, and owner-qualified instantiations; the parser-free checker validates it with V2419. Final gate verification remains. |
| LC-15 | Typed-semantics-to-ELF binding | implemented | Lowering record v2 binds typed entry/block/operation and ABI facts to final machine records; hash/ABI/call/machine mutations fail with V2420. Final gate verification remains. |
| LC-16 | Bounded runtime text | pending | UTF-8 validation, bounded construction/slicing, byte interop, comparison, Molecule layout, cycle limits, and failure semantics. |
| LC-17 | Wide arithmetic strategy | pending | Complete `u128`; either source `u256` or a checked fixed-width library with explicit overflow and cycle behavior. |
| LC-18 | CKB authorization and policy closure | pending | Explicit signer/digest/script-group/witness/replay, capacity, continuity, time, CellDep, and cross-Script composition contracts. |
| LC-19 | Transaction-shaped test closure | pending | Scenario Cells, deps, headers, since, witnesses, exact negative exits, and syscall coverage under selected simulator/CKB-VM backends. |
| LC-20 | Coverage, inspection, debugging, and fuzzing | pending | Source/typed-IR/instruction/action/lock/obligation/error coverage plus bounded parser/type/borrow/checker/lowering corpora. |
| LC-21 | `break`, `continue`, and labels | implemented | Nearest and named loop targets have compile-time validation, explicit CFG jumps, simulator and CKB-VM execution, syntax/formatter/LSP/editor coverage, and unreachable-control diagnostics. Final gate verification remains. |
| LC-22 | Method and index syntax | implemented | Existing collection receiver calls and fixed/bounded indexing use closed type-directed lowering with no new authority or allocation; current executable-surface and syntax matrices cover accepted shapes. Final gate verification remains. |
| LC-23 | Source-local test declarations | pending | `#[test]` and exact expected failures lower into canonical package scenarios and evidence labels. |
| LC-24 | Lambdas and higher-order helpers | pending | Explicit effects, captures, linear ownership, instantiation, borrow escape, and bounded code growth. |
| LC-25 | Typed source macro functions | pending | Bounded, source-mapped, metadata-visible expansion that cannot hide Cell effects. |
| LC-26 | Stable lint and migration UX | pending | Stable rule identities, explicit suppression, CLI/LSP parity, edition-aware migration tests. |

## Stage Gates

### Stage 1: Existing Surface Closure

Scope: LC-01 through LC-05 and parser/type/lowering robustness foundations from
LC-20.

Exit: every documented current production construct either has complete
lowering plus CKB-VM evidence or is rejected before artifact generation.

### Stage 2: Parameterized Value Kernel

Scope: LC-06 through LC-09, LC-16, and LC-17.

Exit: compiler, checker, LSP, docs, metadata, and layouts independently agree
on a broad generic-instantiation and value-ability matrix.

### Stage 3: Modules And Ownership Across APIs

Scope: LC-10 through LC-13.

Exit: package interfaces have canonical identities, reusable APIs preserve
ownership rules, and upgrade reports classify every public semantic change.

### Stage 4: Independent Typed Verification

Scope: LC-14 and LC-15.

Exit: the standalone checker rejects compiler-authored semantic corruption and
binds its recomputed facts to the structural ELF record.

### Stage 5: CKB Libraries And Product Closure

Scope: LC-18 through LC-26.

Exit: representative token, NFT, DAO, covenant, AMM/order,
multisig/authorization, and cross-Script packages require no undocumented glue,
and all release evidence remains accurately labelled.

## Mandatory Final Audit

Before 0.25 can be called complete:

1. every LC row is `verified` with a tracked evidence link;
2. the generated surface matrix is fresh and contains no production
   fail-closed entry;
3. focused type, ownership, compatibility, checker, and CKB-VM suites pass;
4. `./scripts/cellscript_gate.sh dev`, `ci`, and `backend` pass for the final
   tree, with release modes run only when their external prerequisites are
   available;
5. `git diff --check` passes, including initialized submodules; and
6. release notes distinguish compiler, structural-checker, simulator, CKB-VM,
   devnet, deployment, commitment, and mainnet evidence.

Until all six conditions hold, 0.25 remains an implementation line and makes
no complete-language or production-equivalence claim.
