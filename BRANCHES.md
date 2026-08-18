# Branch Context

## nightly-0.25

`nightly-0.25` is the active language-completeness development line. It adds
bounded value generics, explicit visibility and package interfaces, exhaustive
IR-surface classification, and the typed-semantics v2 / lowering-record v3
boundary. Treat it as merge-ready only when compiler, independent checker,
Registry, editor/Playground, docs, and the `dev`, `ci`, and `backend` gates all
agree. It is not a stable release or production CKB evidence claim; the crate
version remains pinned until the coordinated release gate changes it.

## 0.12-era proposal baseline

The 0.12-era work is the formal proposal baseline for grant-style acceptance
discussions. Do not use that historical baseline to describe the current
`main` branch state.

## nightly-0.24

`nightly-0.24` is the active development line for independently verified
artifacts and executable package evidence. It builds on the closed 0.23
Edition 2026 and native-tooling boundary. Treat the line as merge-ready only
when compiler, checker, Registry worker, executable tests, source maps, docs,
and the `dev`, `ci`, and `backend` gates agree. A passing merge gate is not a
stable-release or production CKB claim; those still require the release gate
and the external evidence named in the 0.24 release notes.

## nightly-0.23

`nightly-0.23` is the implementation-complete predecessor for Edition 2026,
resolved target/assurance/ABI/schema profiles, the deployed Registry path, and
the native release-tooling migration. It deliberately rejects older package,
lock, deployment, receipt, builder, and raw entry-witness identities rather
than migrating them. Its release notes are a development-scope record, not a
stable release certificate or production CKB evidence.

## nightly-0.22

`nightly-0.22` is the historical implementation line for the 0.22 type-and-set
theory roadmap. The stable release boundary is the `v0.22.0` tag, not the
nightly branch name.

## main

`main` is the integration baseline. Use an exact release tag for stable-release
comparisons and an exact nightly branch for development-scope comparisons;
do not infer release evidence from `main` alone.

## v0.22.0

`v0.22.0` is the current stable release baseline. Use the exact tag ref
`refs/tags/v0.22.0` for stable comparisons; later nightly branches describe
development work and do not supersede that stable boundary by themselves.

## 0.16

0.16 is an audit-hardening preview. It is useful for tracing how earlier review
findings were handled, but it should not be treated as the current iCKB
differential-evidence branch.

## research/protocol-equivalence

`research/protocol-equivalence` is the 0.17 research and differential-evidence
branch. It moves the iCKB benchmark from model-only evidence into broad partial
CKB VM differential evidence for selected normalized fixtures.

Current active matrix counts:

- `DIFFERENTIAL_CKB_VM_EXECUTED`: 66
- `CELL_SCRIPT_CKB_VM_EXECUTED`: 14
- `ORIGINAL_ICKB_CKB_VM_EXECUTED`: 8
- `MODEL`: 0

The branch still keeps `equivalence_status = NOT_PROVEN` and
`production_equivalence_claim = false`. Do not describe it as production
equivalent until the gate has complete evidence-manifest closure and the
non-executable assumptions registry is empty.
