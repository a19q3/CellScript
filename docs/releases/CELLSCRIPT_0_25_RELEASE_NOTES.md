# CellScript 0.25 Development Release Notes

**Status**: implementation candidate on `nightly-0.25`; final release gates
remain required

CellScript 0.25 concentrates on language completeness where it matters for CKB:
reusable fixed values, explicit package contracts, complete executable
lowering, and an independently checkable typed boundary. It does not attempt to
copy Move's object runtime or claim complete source-to-machine equivalence.

## Starting From The 0.24 Trust Boundary

0.25 includes the complete 0.24 line rather than branching around it. That
means the lock-authoritative package graph, standalone artifact checker,
verified lowering/source-map bundle, explicit simulator and CKB-VM package
scenarios, LS-IDL Registry profile, audited internal assembler, modular
code-generator layout, and production/testnet website parity remain the
baseline for the new language work.

The compiler, standalone checker and adapters now carry the 0.25 package
identity, as do their lockfiles and exact internal dependency pins. The VS Code
extension follows the same 0.25 authoring surface. The independently deployed
Registry Type Script remains versioned as 0.24.0: its byte-for-byte reproducible
artifact and CKB data hash are an already published trust identity, not a
workspace package to relabel. The website continues to advertise `v0.24.0` as
the latest stable release while its development Playground loads the separately
identified 0.25 metadata compiler. A nightly compiler bundle is not presented
as a stable release. The development bundle uses asset identity
`20260824-v0.25.0-32dc571c`, SHA-256
`32dc571c2e8e32134460cb45e2329ddd29d754959d9cfe4478c638aa5fc4c7d7`,
and is 590,724 bytes under standard gzip.

## Language Kernel

- Generic structs, enums, and pure functions now monomorphize deterministically
  before IR lowering. Explicit value abilities and phantom parameters are kept
  separate from Cell lifecycle capabilities, and ordinary generic containers
  cannot hide Cell-backed values. Imported public templates specialize in
  their owning module, including when the consumer uses aliases.
- Built-in `Option<T>`, generic fixed arrays, full-range `u128` literals,
  complete `u128` division/remainder, and integer bitwise and shift operators
  are available under checked width and budget rules.
- Recursive enum, tuple, and struct patterns, binding-free or-patterns,
  field-path borrowing and canonical-root reborrowing now share the same typed
  lowering boundary.
- `break`, `continue`, and `label name: for/while` lower to explicit CFG jumps;
  invalid or unknown targets are compile-time errors.

## Public Interfaces And Upgrades

Top-level declarations accept `public`, `public(package)`, and `private`. Every
compile emits a canonical `cellscript-package-interface-v2` record and
`interface_hash`. `cellc interface` writes it; `cellc interface-diff` compares
source API, serialized layout, runtime ABI, effects/capabilities, generated
builders, and deployment contracts. Breaking reports use `E2501`.

Concrete monomorphizations are implementation evidence, not public exports.
Changing only a private generic use site does not change the interface hash or
produce a breaking report. Edition 2026 remains source compatible; a module
that mixes explicit and implicit visibility emits `W2500` instead of silently
changing the default.

CellScript Registry publication now signs and stores the exact interface. The
API recomputes the hash and rejects incompatible upgrades before admission;
the standalone Registry verifier checks the stored binding again.

## Independently Checked Typed Semantics

Metadata schema 61 adds `cellscript-typed-semantics-v2` and its hash. The record
contains canonical types, layouts, locals, operations, CFG, calls, effects,
ownership, borrow regions, and owner-qualified generic instantiations. Verified
lowering record v3 accounts for every typed block and binds materialized block
hashes to entry ABI and final machine blocks.

The parser/resolver/codegen-independent artifact checker validates the typed
record with `V2419` and its machine binding with `V2420`. Mutation tests cover
both classes. A successful report remains structural and typed verification,
not CKB-VM execution, deployment evidence, or a general semantic-equivalence
claim.

## Playground Experience Upgrade

The Playground remains a metadata-only browser compiler and does not pretend
to emit ELF. Its highlighter now understands generic abilities, visibility,
bitwise/shift operators, complete patterns, borrows, and labeled loop control.
The compiler returns a bounded authoring summary for the Cell Flow, action,
type, and raw-metadata panels. Full public-interface, typed-semantics,
ProofPlan, and verified-artifact records remain native outputs; omitting their
construction and serialization, together with the optional browser semantic
language service, keeps the default WASM bundle inside the 600 KB gzip budget.
The VS Code extension retains the full completion, hover, definition, and
native-report workflow.

## Generated Surface And Failure Discipline

A compiler-owned, exhaustively matched IR-surface registry generates Markdown
and JSON support matrices. Strict production compilation rejects a known
incomplete shape before ASM or ELF generation with `E2105`; fail-closed runtime
helpers remain defense in depth rather than a public feature boundary. An
unregistered IR variant fails compilation, and an unregistered runtime feature
fails closed under every policy.

## Bounded Lifecycle Collections Fail Closed

An audit of `consume_each` and `create_each` found a serious lowering gap: the
frontend and ownership checker accepted the bounded body, but IR replaced it
with `Unit`. A body containing `require false` could therefore compile to an
action that returned success without scanning, consuming, checking, or
creating anything.

0.25 removes that path. The typed IR retains the predicates and create
template and contains an explicit fail-closed call. Metadata reports
`actual_scanned_cardinality:not-observed-no-runtime-lowering`; it does not
claim that a helper ran. The entry ABI marks `BoundedCellSet<T, N>` and
`BoundedList<P, N>` unsupported. A non-production artifact returns stable
runtime error 24 (`collection-runtime-unsupported`) in CKB-VM, while
`--production` and `--deny-fail-closed` return E2105 before writing ASM or ELF.
The diagnostic includes the operation, source origin, missing ProofPlan tier,
and a concrete remediation.

This is a safety boundary, not positive runtime support. The language still
needs a canonical transaction/group selection rule and Type Script identity
for `BoundedCellSet`, plus a witness element codec, output order and
one-to-one correspondence, fresh identity, and capacity rules for
`BoundedList`/`create_each`. Builder evidence is not accepted as a replacement
for those on-chain checks.

This distinction is intentional: 0.25 ships the safety fix and the audit trail,
not invented consensus semantics. Positive bounded collection execution can be
added only after those choices are specified and covered by exact positive and
adversarial CKB-VM vectors.

## Bounded Local Evidence Storage

Compiler caches and local gate evidence no longer grow without a default
bound. Incremental caches keep the 32 most recently used identities per root;
managed syntax, strict-backend, and CKB-acceptance streams keep three runs per
mode. Successful syntax audits discard reproducible per-case intermediates,
and production acceptance removes its transient Cargo target and stopped-node
database while preserving the reports and verified artifacts that identify the
run. Matching large files are hardlinked only after SHA-256 comparison.

`latest-<mode>.json` indexes retain the exact report path, hash, size, and
status. Operators can override the bounds for an external archiver or a debug
session. Automatic retention and `cellc clean --cache` are confined to their
workspace/evidence roots and reject symlinked managed path components instead
of following them outside the repository. Cache hits accept only regular
payload files and refresh recency through a newly created temporary file plus
rename; cache stores create a fresh entry and never overwrite a pre-populated
entry. A package-provided `.last_used` or payload symlink therefore cannot turn
an ordinary compile into a write outside the cache.

## What 0.25 Does Not Claim

- `BoundedCellSet` and `BoundedList` are not positive production runtime
  features yet. Their frontend and audit representations remain useful, but an
  executable use is deliberately rejected as described above.
- Typed-record verification is not a proof of general source-to-machine
  semantic equivalence and is not CKB-VM, deployment, commitment, or mainnet
  evidence.
- The browser compiler remains metadata-only and does not emit an ELF.
- A development package version, branch, website build, or passing local gate
  is not a stable release. Stable publication still requires the coordinated
  release process and an exact tag.

## Release Validation

The final 0.25 candidate is closed with the repository's complete development,
CI, and backend gates:

```bash
./scripts/cellscript_gate.sh dev
./scripts/cellscript_gate.sh ci
./scripts/cellscript_gate.sh backend
git diff --check
```

Focused compiler, checker, Registry and CKB-VM tests, the VS Code extension
validation, the website production/testnet build, and a complete security diff
review are part of that closure. These checks establish merge readiness for
the nightly line. Release and chain evidence remain separate from local gates.
