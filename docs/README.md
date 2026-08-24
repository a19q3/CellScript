# CellScript Documentation Map

This directory is organized by document role. Keep new docs in the smallest
stable category that matches how readers should use them.

## Stable Tutorials

`docs/wiki/` contains the GitHub Wiki source. These pages are version-neutral,
reader-facing tutorials and cookbook material. They should teach the current
stable surface rather than act as release logs.

## Release Records

`docs/releases/` contains finalized release notes and active release-note
drafts. Released versions should use non-draft filenames.

- `docs/releases/CELLSCRIPT_0_13_2_RELEASE_NOTES.md` is the final 0.13.2
  release note and the canonical 0.13 release summary.
- `docs/releases/CELLSCRIPT_0_13_RELEASE_SCOPE.md` records the closed 0.13
  implementation scope and release evidence boundary.
- `docs/releases/CELLSCRIPT_0_13_2_ACCEPTANCE_COMMUNITY_POST.md` is a
  community-facing summary of the 0.13.2 CKB acceptance and stateful flow
  evidence.
- `docs/releases/CELLSCRIPT_0_14_RELEASE_NOTES.md` is the final 0.14.0 release
  note and release-evidence summary.
- `docs/releases/CELLSCRIPT_0_15_RELEASE_NOTES.md` is the final 0.15.0 release
  note and release-evidence summary.
- `docs/releases/CELLSCRIPT_0_16_RELEASE_NOTES.md` is the initial 0.16.0 release
  note and release-evidence summary.
- `docs/releases/CELLSCRIPT_0_16_1_RELEASE_NOTES.md` is the final 0.16.1 release
  note and release-evidence summary.
- `docs/releases/CELLSCRIPT_0_20_RELEASE_NOTES.md` records the generated-builder
  and live-registry line.
- `docs/releases/CELLSCRIPT_0_21_RELEASE_NOTES.md` records semantic closure,
  authenticated evidence, the canonical CLI tree, MCP, and skill-pack scope.
- `docs/releases/CELLSCRIPT_0_22_RELEASE_NOTES.md` is the final stable 0.22
  record for its typed language, diagnostics, metadata schema 55, and bounded
  Fiber boundary.
- `docs/releases/CELLSCRIPT_0_23_RELEASE_NOTES.md` is the final stable 0.23
  record for Edition 2026, resolved compatibility profiles, metadata schema
  57, recoverable browser tooling, and the Registry publisher-session flow.
- `docs/releases/CELLSCRIPT_0_24_RELEASE_NOTES.md` is the stable 0.24 record
  for metadata schema 58, independently checked ELF/lowering evidence,
  executable package scenarios, and the least-privilege Registry artifact
  worker.
- `docs/releases/CELLSCRIPT_0_25_RELEASE_NOTES.md` is the active implementation
  record for metadata schema 61, bounded generics, public interfaces,
  typed-semantics verification,
  executable-surface closure, bounded-collection fail-closed hardening, local
  evidence retention, and the upgraded Playground inspector.

Release candidates and planning notes should not live here unless they are the
final release record.

## Reference And Evidence Contracts

Top-level `docs/CELLSCRIPT_*.md` files are active reference material when they
describe current compiler behavior, target-profile evidence, runtime errors,
syntax governance, metadata, capacity, deployment manifests, or support
matrices.

High-value active references include:

- `releases/CELLSCRIPT_0_13_2_RELEASE_NOTES.md` for the final 0.13 syntax
  governance summary
- `CELLSCRIPT_GATE_POLICY.md`
- `CELLSCRIPT_SYNTAX_COMBO_AUDIT_METHODOLOGY.md`
- `CELLSCRIPT_GRAMMAR_GOVERNANCE_RFC.md` for the active grammar-governance
  direction around transition shape, `verification`, `require`, and accounting
  syntax
- `CELLSCRIPT_SURFACE_ELEGANCE_RFC.md` for deferred syntax candidates that
  require full parser/typechecker/lowering/metadata/formatter/LSP coverage
- `CELLSCRIPT_CAPACITY_AND_BUILDER_CONTRACT.md`
- `CELLSCRIPT_CKB_ADAPTER.md`
- `CELLSCRIPT_CELLFABRIC_BRIDGE.md`
- `CELLSCRIPT_PACKAGE_PROVENANCE_AND_DEPLOYMENT_IDENTITY.md`
- `CELLSCRIPT_REGISTRY_PRODUCTION_BOUNDARY_ADR.md` for the accepted production
  boundary of the wallet-rooted public registry write/read architecture and
  isolated Pudge testnet sandbox
- `CELLSCRIPT_LS_IDL_REGISTRY_PROFILE.md` for byte-exact LS-IDL admission,
  executable suffix commitment, Script-identity lookup, tooling, and operator
  boundaries
- `../services/registry-api/README.md` for the Cloudflare Workers + R2 + Neon
  write API implementation and deployment checklist
- `CELLSCRIPT_COLLECTIONS_SUPPORT_MATRIX.md`
- `CELLSCRIPT_ENTRY_WITNESS_ABI.md`
- `CELLSCRIPT_EXAMPLE_BUSINESS_FLOWS.md`
- `CELLSCRIPT_LINEAR_OWNERSHIP.md`
- `CELLSCRIPT_OUTPUT_BINDINGS.md`
- `CELLSCRIPT_RUNTIME_ERROR_CODES.md`
- `CELLSCRIPT_VERIFIED_ARTIFACT_BOUNDARY.md`
- `CELLSCRIPT_PUBLIC_INTERFACES.md`
- `CELLSCRIPT_EXECUTABLE_TEST_SCENARIOS.md`
- `CELLSCRIPT_MYELIN_0_24_HANDOFF.md`
- `CELLSCRIPT_COMPILER_ERROR_CODES.md`
- `CELLSCRIPT_SCHEDULER_HINTS.md`
- `../examples/fiber/README.md` for the bounded 0.22 Fiber interoperability
  operator workflow

## Specs And Future Tracks

- `docs/spec/` contains normative or semi-normative specifications. The 0.16
  operational semantics live there.
- `docs/0.20/` now keeps only release-facing evidence material for the 0.20
  line, including `CELLSCRIPT_PROTOCOL_MULTI_FILE_EVIDENCE.md` for the
  evidence-gated NovaSeal/iCKB/DobEvo protocol-source boundary. Superseded
  0.20 audit notes have been removed from the main branch.

## Examples

`docs/examples/` contains focused example notes and matrices that support the
bundled `.cell` examples. These are not release notes.

- `docs/examples/token_amm_bootstrap.md` records the concrete token authority
  bootstrap and AMM builder path for the bundled `launch`, `token`, and
  `amm_pool` examples.

## Design And Release Records

Shipped behavior is recorded in `releases/`, while forward-looking work must
be owned by a concrete RFC under `docs/` or a scoped implementation proposal
under `proposals/`. The documentation set does not maintain a separate roadmap
layer.

Active design and evidence records include:

- `CELLSCRIPT_CKB_STD_COMPAT.md` for the ckb-std compatibility boundary
- `CELLSCRIPT_GRAMMAR_GOVERNANCE_RFC.md` and
  `CELLSCRIPT_SYNTAX_COMBO_AUDIT_METHODOLOGY.md` for 0.19 grammar/syntax
  governance scope
- `CELLSCRIPT_REGISTRY_PHASE1.md` for the current artifact, verification,
  deployment-evidence, and public API contract
- `CELLSCRIPT_LS_IDL_REGISTRY_PROFILE.md` for the 0.24 Lock Script interface
  profile and compatibility evidence
- `releases/CELLSCRIPT_0_22_RELEASE_NOTES.md` for the typed transaction-view,
  bounded collection, validity, borrow, capability, payload-enum, and Fiber
  release boundary
- `releases/CELLSCRIPT_0_23_RELEASE_NOTES.md` and
  `releases/CELLSCRIPT_0_24_RELEASE_NOTES.md` for the Edition/ABI, Registry,
  verified-artifact, executable-test, and ecosystem evidence boundaries

## Archive

`docs/archive/` contains historical plans and superseded execution documents.
Archived files may remain useful for design archaeology, but they are not the
current stable contract.

Current archive:

- `docs/archive/0.13/CELLSCRIPT_0_13_1_PLAN.md`
- `docs/archive/0.13/CELLSCRIPT_SIGNATURE_DIRECTION_EXECUTION_PLAN.md`
- `docs/archive/0.17/CELLSCRIPT_0_17_ICKB_FINAL_REPORT.md`
- `docs/archive/0.17/CELLSCRIPT_0_17_ICKB_PRODUCTION_EQUIVALENCE_GATE.md`
- `docs/archive/0.17/CELLSCRIPT_0_17_REVIEW_FINDINGS_CLOSURE.md`

When moving a document into the archive, update all public links and add a short
status note if the file could otherwise be mistaken for active guidance.
