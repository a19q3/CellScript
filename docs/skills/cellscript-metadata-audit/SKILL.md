---
name: cellscript-metadata-audit
description: CompileMetadata, ProofPlan, builder assumptions, constraints, ABI, audit bundles, receipts, and artifact verification.
references:
  - docs/wiki/Tutorial-06-Metadata-Verification-and-Production-Gates.md
  - docs/wiki/Tutorial-11-Scoped-Invariants-and-ProofPlan.md
  - docs/wiki/Tutorial-14-Verified-Artifacts-and-Executable-Tests.md
  - docs/CELLSCRIPT_GATE_POLICY.md
  - docs/CELLSCRIPT_VERIFIED_ARTIFACT_BOUNDARY.md
commands:
  - cellc metadata
  - cellc constraints
  - cellc explain proof
  - cellc audit-bundle
  - cellc verify-artifact
  - cellc verify-receipt
---

# CellScript Metadata Audit

Use this skill when reviewing compiler evidence. Treat metadata as an audit
stream, not consensus truth. ProofPlan rows, TemplateLayout records, receipts,
constraints, ABI, and builder assumptions explain what the compiler emitted and
what remains to be checked by builders or CKB nodes.

For the current 0.25 development line, inspect current metadata schema 60 under
Edition 2026 and the resolved compatibility profile, together with the canonical
lowering record and source map for CKB ELF builds. Typed transaction views, bounded
quantifiers/collections, capability proofs, enum layouts, validity predicates,
borrow regions, and `fungible-type-group-v1` evidence introduced on the 0.22
line remain part of that evidence stream. The 0.25 value-generic kernel adds
`generic_instantiations` with canonical source identities, concrete internal
names, type arguments, and the closed value-ability registry.

Distinguish evidence states precisely: compile-only, metadata-only,
runtime-required, helper-backed, builder-backed, node dry-run, tx-pool accepted,
submitted, and externally attested.

Validation defaults:

- run `cellc metadata . --target-profile ckb` to inspect metadata without
  writing a file;
- run `cellc explain proof . --target-profile ckb --json` for ProofPlan;
- run `cellc verify-artifact` before trusting the artifact/metadata/lowering/
  source-map identity and structural contract;
- keep the report's binding, structural, lowering-record, CKB-VM, chain, and
  semantic-equivalence fields separate.
