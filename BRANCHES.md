# Branch Context

## nightly-0.25

`nightly-0.25` is the closed language-completeness release line. It adds
bounded value generics, explicit visibility and package interfaces, exhaustive
IR-surface classification, and the typed-semantics v2 / lowering-record v3
boundary. The stable release boundary is the exact `v0.25.0` tag; later
commits on the branch are not implicitly part of that release. External
Myelin, Fiber, and RGB++ claims remain separately evidence gated as described
in the 0.25 release notes.

## 0.12-era proposal baseline

The 0.12-era work is the formal proposal baseline for grant-style acceptance
discussions. Do not use that historical baseline to describe the current
`main` branch state.

## nightly-0.24

`nightly-0.24` is the closed maintenance line for independently verified
artifacts and executable package evidence. It builds on the closed 0.23
Edition 2026 and native-tooling boundary. The stable release boundary is the
exact `v0.24.0` tag; later commits on the branch are not implicitly part of
that release. External Myelin, Fiber, and RGB++ claims remain separately
evidence gated as described in the 0.24 release notes.

## nightly-0.23

`nightly-0.23` is the implementation-complete predecessor for Edition 2026,
resolved target/assurance/ABI/schema profiles, the deployed Registry path, and
the native release-tooling migration. It deliberately rejects older package,
lock, deployment, receipt, builder, and raw entry-witness identities rather
than migrating them. Its release notes are a development-scope record, not a
stable release certificate or production CKB evidence.

## nightly-0.22

`nightly-0.22` is the historical implementation line for the 0.22 type-and-set
theory release. The stable release boundary is the `v0.22.0` tag, not the
nightly branch name.

## main

`main` is the integration baseline. Use an exact release tag for stable-release
comparisons and an exact nightly branch for development-scope comparisons;
do not infer release evidence from `main` alone.

## v0.25.0

`v0.25.0` is the current stable release for the language-completeness kernel,
canonical package interfaces, independently checked typed semantics, verified
lowering records, and the 0.25 Playground compiler. Use the exact tag ref
`refs/tags/v0.25.0` for stable comparisons. The release does not promote the
separately pending Myelin, Fiber, or RGB++ external evidence boundaries.

## v0.24.0

`v0.24.0` is the historical stable predecessor for the verified-artifact checker,
executable package scenarios, lock-authoritative package graph, and LS-IDL
Registry path. Use the exact tag ref `refs/tags/v0.24.0` for stable
comparisons. The release does not promote the separately pending Myelin,
Fiber, or RGB++ external evidence boundaries.

## v0.23.0

`v0.23.0` is the historical stable baseline for Edition 2026, the Registry,
and native release tooling. Use the exact tag ref `refs/tags/v0.23.0` when
reproducing that release.

## v0.22.0

`v0.22.0` is the historical stable baseline for the type-and-set-theory line.
Use the exact tag ref `refs/tags/v0.22.0` when reproducing that release rather
than treating a later nightly branch as equivalent evidence.

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
