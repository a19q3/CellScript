# CellScript 0.25: More Expressive Contracts, Safer Upgrades, Stronger Verification

**Publication status**: Draft. Publish only after the exact `v0.25.0` tag has
passed the coordinated release process and the repository identifies 0.25 as
the current stable release.

CellScript 0.25 is here. This release makes everyday contract code easier to
reuse, package upgrades easier to review, and the path from typed source to
RISC-V ELF easier to check independently.

If you write CellScript, the visible changes are generics, `Option<T>`, richer
patterns, full-width `u128` operations, explicit visibility, field-path
borrowing, and labeled loop control. If you maintain or audit packages, the
deeper changes are canonical public interfaces, six-dimensional compatibility
reports, typed-semantics records, and new independent checker coverage.

0.25 also fixes an unsafe bounded-collection lowering gap. An unsupported
operation can no longer disappear during compilation and leave behind a false
success path. It now fails explicitly.

## The Short Version

- Write reusable fixed-width value code with bounded generics, generic structs,
  enums, pure functions, fixed arrays, and `Option<T>`.
- Express more logic directly with full-range `u128` literals, checked
  division and remainder, bitwise and shift operators, recursive patterns,
  field-path borrows, and labeled `break` / `continue`.
- Mark package declarations `public`, `public(package)`, or `private`, then
  compare releases across source API, layout, ABI, effects, builders, and
  deployment contracts.
- Inspect a canonical typed-semantics record and have the independent artifact
  checker bind it through lowering to the final RISC-V ELF.
- Get an explicit error for unsupported executable surfaces instead of an
  artifact that appears to support them.
- Use the upgraded Playground and VS Code authoring surfaces without turning
  the browser compiler into an ELF builder.
- Keep local compiler caches and gate evidence from growing without a default
  bound.

## For Contract Authors: More Reuse, Same Explicit Cell Ownership

### Bounded generics for ordinary values

CellScript can now specialize generic value structs, enums, and pure functions
before IR lowering:

```cellscript
struct Pair<T: copy + drop + store + fixed + serializable + non_linear>
    has copy, drop, store, fixed, serializable, non_linear
{
    left: T,
    right: T,
}

fn first<T: copy + drop + store + fixed + serializable + non_linear>(
    pair: Pair<T>,
) -> T {
    pair.left
}
```

The same checked path supports imported public templates. A package can use an
alias for a dependency and specialize its generic types or functions while the
specialization remains owned by the module that declared the template.

The boundary is intentionally narrow:

- generic instantiation is deterministic and budgeted;
- value abilities and phantom parameters are explicit;
- abilities such as `copy`, `drop`, and `store` do not grant Cell lifecycle
  authority;
- ordinary generic containers cannot hide a `resource`, `shared`, or other
  Cell-backed value.

This gives users reusable value code without introducing an object runtime or
weakening CellScript's linear ownership model.

### Useful built-ins and complete fixed-width operations

0.25 adds or completes:

- built-in `Option<T>` using the checked fixed-width generic enum path;
- generic fixed arrays;
- decimal `u128` literals across the full `u128` range;
- checked `u128` division and remainder;
- integer `&`, `|`, `^`, `<<`, and `>>` lowering;
- exact scalar and wide division-by-zero guards.

Wide operands now use a shared stack-spilled loading path, so resolving a
dynamic Molecule field cannot overwrite a live `u128` limb. Constant folding
also refuses arithmetic that would wrap even though the runtime operation
would trap. Dynamic-schema CKB-VM vectors cover `u128` addition, bitwise
operations, and shifts.

### Patterns that work beyond the simplest case

`match` now supports recursive enum, tuple, and struct patterns, plus
binding-free or-patterns. Fixed tuple and array values can be materialized and
projected through the paths needed by nested payload matches.

Exhaustiveness checking uses a bounded constructor-matrix computation. Nested
and or-pattern coverage is merged instead of being approximated from the top
level, making useful matches accepted and incomplete ones rejected before
code generation.

### Clearer borrowing and loop control

Read-only Cell views can borrow a field path and reborrow from the canonical
root. The compiler still rejects escaping the view, crossing a consume or
destroy operation, or using the borrow as persistent storage.

Loops now support `break`, `continue`, and `label name: for/while`. Valid
targets lower to explicit control-flow graph edges; an unknown or invalid
target is a compile-time error.

## For Package Maintainers: Know What An Upgrade Changes

Top-level declarations can now be marked:

```text
public
public(package)
private
```

Every successful compile emits a canonical
`cellscript-package-interface-v2` record and `interface_hash`. Edition 2026
remains source compatible. If a module mixes explicit and implicit visibility,
the compiler emits `W2500` instead of silently changing the default.

Generate an interface with:

```bash
cellc interface path/to/package --output target/package.interface.json
```

Compare two versions with:

```bash
cellc interface-diff \
  --old target/old.interface.json \
  --new target/new.interface.json
```

The report separates six kinds of compatibility:

1. source API;
2. serialized layout;
3. runtime ABI;
4. effects and capabilities;
5. generated builders;
6. deployment contracts.

A breaking report exits with stable diagnostic `E2501`. Additive exports still
change the interface hash, but they are reported as compatible.

Concrete monomorphizations are implementation evidence, not public exports.
Changing a private generic use site therefore does not create a public API
break. Registry publication signs and stores the canonical interface, the API
recomputes its hash and checks upgrade compatibility at admission, and the
standalone Registry verifier checks the stored binding again.

## For Auditors: Follow Typed Intent To The Machine Artifact

0.24 introduced an independently checked four-file artifact bundle:

```text
contract.elf
contract.elf.meta.json
contract.elf.lowering.json
contract.elf.sourcemap.json
```

0.25 moves that boundary closer to the programmer's intent. Metadata schema 61
adds `cellscript-typed-semantics-v2`, covering:

- canonical types and layouts;
- locals and operations;
- control-flow blocks and calls;
- effects and ownership;
- borrow regions;
- owner-qualified generic instantiations.

Verified lowering record v3 accounts for each typed block and binds its
materialized hash to the entry ABI and final machine blocks. In plain language,
the evidence chain is:

```text
typed program -> lowering blocks -> entry ABI -> final RISC-V machine ranges
```

The standalone `cellscript-artifact-checker` verifies that chain without
loading the parser, resolver, type checker, IR, optimizer, assembler, or code
generator. Its new stable rejection classes are:

- `V2419`: malformed or inconsistent typed semantics;
- `V2420`: a broken typed-record, lowering, ABI, or machine binding.

Deterministic mutation tests cover both classes. This makes compiler drift and
artifact tampering easier to detect independently.

The claim remains precise: this is bounded structural and typed verification.
It is not a general proof of source-to-machine semantic equivalence, CKB-VM
execution, deployment, commitment, or mainnet acceptance.

## One Registry For Every Executable Surface

0.25 adds a compiler-owned registry of the IR and runtime surfaces that can
reach an executable artifact. It generates both Markdown and JSON support
matrices and is exhaustively matched by the compiler.

That changes failure behavior in two important ways:

- a known but incomplete executable shape is rejected in strict production
  compilation with `E2105` before ASM or ELF is written;
- a new IR variant or runtime feature cannot silently bypass the registry—it
  must be classified or compilation fails.

Fail-closed runtime helpers remain defense in depth. They are no longer used to
make an unsupported feature look production-ready.

## An Important Safety Fix For Bounded Collections

During the 0.25 audit, we found a serious gap in `consume_each` and
`create_each`.

The frontend and ownership checker accepted the bounded operation, but IR
lowering replaced its body with `Unit`. That meant a body containing
`require false` could compile into an action that returned success without
scanning, checking, consuming, or creating anything.

0.25 removes that path:

- typed IR retains the predicate or create template;
- lowering inserts an explicit registered fail-closed call;
- permissive artifacts return stable runtime error 24,
  `collection-runtime-unsupported`, in CKB-VM;
- `--production` and `--deny-fail-closed` stop with `E2105` before producing
  ASM or ELF;
- the diagnostic names the operation, source location, missing ProofPlan tier,
  and a concrete remediation;
- entry ABI and ProofPlan metadata no longer claim supported pointers or
  runtime-observed cardinality when no runtime scan exists.

This is a safety fix, not positive runtime support for bounded lifecycle
collections. The remaining work needs exact transaction-group selection,
canonical Cell and witness decoding, output order and one-to-one
correspondence, identity rules, capacity rules, and positive and adversarial
CKB-VM vectors. 0.25 does not invent those consensus semantics.

## For Playground And VS Code Users

The browser Playground now understands the 0.25 authoring surface:

- generics, abilities, and visibility;
- bitwise and shift operators;
- recursive patterns;
- field-path borrows;
- labeled loop control.

The compiler returns a bounded authoring summary for the Cell Flow, action,
type, and raw-metadata panels. Full public-interface, typed-semantics,
ProofPlan, and verified-artifact records remain available through native
`cellc` and the VS Code workflow.

The browser compiler remains metadata-only and does not emit ELF. Omitting
native-only records and the optional browser semantic language service keeps
the default WASM bundle within its 600 KB gzip budget. The VS Code extension
retains full completion, hover, definition, and native-report workflows.

## For CI And Local Development

Compiler caches and gate evidence now have bounded defaults:

- incremental caches keep the 32 most recently used identities per root;
- managed syntax, strict-backend, and CKB-acceptance streams keep three runs
  per mode;
- successful syntax audits discard reproducible per-case intermediates;
- production acceptance removes its transient Cargo target and stopped-node
  database while retaining identifying reports and verified artifacts;
- identical large files are hardlinked only after their SHA-256 identities
  match;
- `latest-<mode>.json` records the exact report path, hash, size, and status;
- `cellc clean --cache` also finds nested workspace cache roots.

Cleanup is confined to managed workspace and evidence roots. Symlinked managed
path components and non-regular cache payloads are rejected, cache entries are
created fresh rather than overwritten, and recency updates use create-new plus
rename. Operators can still override the retention bound for an external
archiver or an explicit debugging session.

## Everything 0.24 Established Is Still Here

0.25 is built on the complete 0.24 boundary rather than a parallel branch. It
includes:

- the lock-authoritative package graph;
- exact dependency and environment identities;
- the standalone artifact checker;
- verified lowering and source-map sidecars;
- explicit simulator and CKB-VM package scenarios;
- the LS-IDL Registry profile;
- the audited internal assembler as the ELF path;
- the modular code-generator layout;
- production and Pudge Testnet website parity.

The compiler, checker, adapters, Registry verifiers, lockfiles, and editor move
to the 0.25 package identity. The independently deployed Registry Type Script
does not: it remains the byte-identical 0.24.0 artifact because its package
version and CKB data hash are already part of its published trust identity.

## What 0.25 Does Not Claim

- `BoundedCellSet` and `BoundedList` do not yet have positive production
  runtime support for lifecycle iteration. Unsupported executable uses fail
  closed as described above.
- Typed-record verification is not a proof of complete source-to-machine
  semantic equivalence.
- Artifact verification is not CKB-VM, deployment, commitment, confirmation,
  or mainnet evidence.
- The browser compiler does not emit an ELF.
- The Registry Type Script was not relabeled as 0.25.
- A branch name, package version, website asset, or passing local gate is not a
  stable release without the exact tag and coordinated release process.

## Try CellScript 0.25

Install the exact release:

```bash
curl -fsSL \
  https://github.com/CellScript-Labs/CellScript/releases/download/v0.25.0/install.sh | sh

cellc --version
```

For an existing package, start with the new review surfaces:

```bash
cellc explain generics path/to/package
cellc interface path/to/package --output target/package.interface.json
cellc check --target-profile ckb --all-targets --production
cellc build --target riscv64-elf --target-profile ckb --production
cellc verify-artifact build/main.elf --verify-sources --production
```

When preparing an upgrade, save the old and new interfaces and run
`cellc interface-diff` before publishing.

## Where To Read More

- [Complete 0.25 release notes](CELLSCRIPT_0_25_RELEASE_NOTES.md)
- [Public interfaces and compatibility](../CELLSCRIPT_PUBLIC_INTERFACES.md)
- [Verified artifact boundary](../CELLSCRIPT_VERIFIED_ARTIFACT_BOUNDARY.md)
- [Collections support matrix](../CELLSCRIPT_COLLECTIONS_SUPPORT_MATRIX.md)
- [Metadata verification and production gates](../wiki/Tutorial-06-Metadata-Verification-and-Production-Gates.md)

CellScript 0.25 makes the language more capable without making Cell ownership
implicit. It makes package upgrades comparable without reducing them to a
version number. It makes typed compiler intent independently checkable against
the emitted artifact. And when runtime semantics are not ready, it rejects the
program instead of pretending they ran.

That combination—better authoring, clearer upgrades, stronger evidence, and
fail-closed execution—is the release.

---

## Short Announcement

CellScript 0.25 is out.

For contract authors, this release adds bounded non-Cell generics, `Option<T>`,
generic fixed arrays, full-range `u128` literals, checked division and
remainder, bitwise and shift operators, recursive patterns, field-path borrows,
and labeled loop control.

For package maintainers, explicit `public`, `public(package)`, and `private`
visibility now feeds a canonical package interface. `cellc interface-diff`
checks upgrades across source API, serialized layout, runtime ABI, effects,
builders, and deployment contracts.

For auditors, metadata schema 61 adds canonical typed semantics, and the
independent artifact checker binds that record through lowering to the final
RISC-V ELF with new `V2419` and `V2420` checks.

0.25 also fixes an unsafe bounded-collection lowering gap. Unsupported
`consume_each` and `create_each` paths now fail closed instead of compiling to
a false success. Positive runtime support remains deliberately deferred until
its consensus rules and adversarial CKB-VM evidence are complete.

The Playground and VS Code extension understand the new authoring surface,
while the browser remains an honest metadata-only compiler.

Release notes: https://github.com/CellScript-Labs/CellScript/blob/v0.25.0/docs/releases/CELLSCRIPT_0_25_RELEASE_NOTES.md

Release: https://github.com/CellScript-Labs/CellScript/releases/tag/v0.25.0
