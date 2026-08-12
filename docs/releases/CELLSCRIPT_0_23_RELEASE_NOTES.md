# CellScript 0.23.0 Release Notes

**Release**: [`v0.23.0`](https://github.com/CellScript-Labs/CellScript/releases/tag/v0.23.0),
2026-08-11.

**Release boundary**: the stable-release claim applies to the exact
`v0.23.0` tag. It does not turn compiler or Registry evidence into a claim that
an individual contract is audited or ready for mainnet deployment.

CellScript 0.23.0 is primarily a compatibility and distribution release. The
language surface is mostly unchanged. The significant changes are at the
boundaries between source code, generated artifacts, transaction builders, and
the package Registry:

- every package now declares the source-semantics edition `2026`;
- compiler outputs carry one resolved compatibility-profile identity;
- parameterized CKB entries accept arguments only from
  `WitnessArgs.input_type`;
- older lock, deployment, receipt, builder, and raw-witness identities are
  rejected rather than interpreted as 0.23 data; and
- the public Registry now has a working publish, compiler-verification,
  discovery, and source-package installation path.

Existing projects need a deliberate upgrade. In particular, changing the
package version alone is not sufficient: the manifest, witness builder, and
persisted build records must all move to the 0.23 identities together.

## Install 0.23.0

Install a published binary:

```bash
CELLSCRIPT_VERSION=0.23.0 curl -fsSL https://raw.githubusercontent.com/CellScript-Labs/CellScript/main/scripts/install.sh | sh
cellc --version
```

The GitHub release includes `SHA256SUMS` for the four platform archives.

To build the exact released source, use the repository-pinned Rust 1.97.1
toolchain:

```bash
git clone --branch v0.23.0 --depth 1 https://github.com/CellScript-Labs/CellScript.git
cd CellScript
cargo install --locked --path .
```

A new package created by 0.23 already contains the required edition:

```bash
cellc init hello-cell
cd hello-cell
cellc check --target-profile ckb
cellc build --target riscv64-elf --target-profile ckb
```

CellScript is still in a CKB-focused alpha and stabilisation phase. Mainnet use
requires contract review and transaction-level evidence in addition to a
successful compiler run.

## Upgrading An Existing Package

The minimum source-manifest change is:

```toml
[package]
edition = "2026"
```

Then regenerate the records that are bound to compiler output:

1. rebuild every RISC-V artifact and its metadata;
2. refresh `Cell.lock` and `Deployed.toml` instead of retaining their older
   schemas;
3. regenerate compile receipts and generated action builders;
4. update transaction builders and fixtures to put the `CSARGv1` payload in
   `WitnessArgs.input_type` before signing; and
5. repeat package checks, CKB-VM tests, capacity checks, and any deployment
   verification used by the project.

There is no compatibility reader that upgrades an old persisted record in
place. A missing edition, a non-2026 edition, or an old record identity is an
error. This is intentional: two tools should not be able to read the same
package or deployment record and silently assign different semantics to it.

## Edition 2026 And The Resolved Compatibility Profile

Edition 2026 is CellScript's first source-semantics edition. The year is a
long-lived epoch label, not a promise of annual editions and not a shorthand
for every compiler ABI.

The edition owns source rules that could change the meaning of an unchanged
`.cell` file, including parsing ambiguities, name resolution, typing and
coercion, desugaring, and resource-flow semantics. Other compatibility axes
continue to version independently:

- compiler SemVer;
- target profile;
- primitive-assurance mode;
- entry-payload encoding;
- witness placement and script-group source; and
- metadata, source, artifact, and constraints schemas.

The compiler combines these values into
`cellscript-resolved-compatibility-profile-v1` and emits the resolved profile
and its hash in compile metadata. Tools that consume an artifact compare the
same hash instead of inferring compatibility from the compiler version.

The persisted 0.23 identity set is:

| Surface | 0.23 identity |
| --- | --- |
| Compile metadata | metadata schema 57, source schema 2, artifact schema 1, constraints schema 2 |
| Resolved profile | `cellscript-resolved-compatibility-profile-v1` with independent source, target, assurance, ABI, and schema axes |
| `Cell.lock` | version 2 |
| `Deployed.toml` | version 2 with schema `cellscript-deployed-v0.23-edition-2026` |
| Compile receipt | receipt v2 with edition and resolved profile |
| Generated action builder | `cellscript-generated-action-builder-v0.23-edition-2026` |
| Registry build record | explicit edition and compatibility-profile hash |
| Registry publication | one complete entry containing edition, profile hash, status, dependencies, and yank state |

The same profile identity is carried through the CLI, LSP, native library,
WASM metadata API, Registry, lock file, deployment record, receipt, and builder
output. A mismatch fails at the boundary where it is observed; consumers do
not substitute a default profile.

## Canonical Entry Arguments In `WitnessArgs.input_type`

At the CKB transaction layer, a witness is a byte array. CellScript 0.23
requires the selected witness bytes to encode the standard Molecule
`WitnessArgs` table:

```text
WitnessArgs {
    lock:        BytesOpt,  // Lock Script or signature data
    input_type:  BytesOpt,  // CellScript CSARGv1 entry payload
    output_type: BytesOpt,  // output-side Type Script data
}
```

The placement ABI is `cellscript-witnessargs-input-type-v2`. The generated
entry wrapper performs the following steps:

1. select `GroupInput#0` for the active script group;
2. if the group has no input, select `GroupOutput#0`;
3. validate the `WitnessArgs` table and its `BytesOpt` offsets;
4. extract `input_type`;
5. check the `CSARGv1\0` payload magic; and
6. decode the positional entry arguments.

The wrapper no longer accepts a raw `CSARGv1` byte array as an alias for a
`WitnessArgs` value. It also rejects a missing `input_type`, malformed Molecule
offsets, or a payload placed in `lock` or `output_type`. These cases return
runtime error `25 entry-witness-abi-invalid`.

Generated builders parse or create `WitnessArgs`, preserve existing `lock` and
`output_type` values, and refuse to overwrite an occupied `input_type`. The
payload is placed before the transaction is signed so that the final witness
layout is covered by the signing flow.

Two naming points are worth making explicit:

- `input_type` is a field of `WitnessArgs`; it does not mean the Type Script of
  an input Cell.
- `CSARGv1` remains CellScript's entry-payload encoding inside that field; it
  does not replace Molecule or the CKB `WitnessArgs` convention.

This placement leaves `lock` available for Lock Script signatures and keeps
CellScript arguments separate from output-side Type Script data.

## Publishing Through The Public Registry

The public Registry is available at
[cellscript.dev/registry](https://cellscript.dev/registry/). In 0.23, the
source-package path is connected from the CLI through the write API and
compiler worker to public discovery and installation.

Before publishing, verify the package and inspect the request without writing:

```bash
cellc package verify --json
cellc publish --dry-run
```

For the first publication under a package coordinate, run:

```bash
cellc publish --authorise
```

The CLI creates a delegated P-256 publishing key, stores it as pending in the
local operating-system keychain, and creates a 15-minute browser session for
the exact namespace, package, and artifact kind. After wallet approval, the
Registry returns the matching key ID, the CLI marks the local key active, and
the original publish continues. `--no-open` prints the session URL for remote
or terminal-only use.

The private publishing key does not move into the browser. The browser approves
the delegated capability; later releases can use the active local key until its
capability expires or is revoked. The explicit `auth capability submit` and
`auth namespace claim` sequence remains available for CI, manual signing, and
external-wallet workflows.

## What Registry Verification Means

A successful publish first admits a signed source record and immutable source
snapshot. Admission also creates a compiler-verification job. A separate
least-privilege worker then:

1. authenticates the snapshot descriptor and source contents;
2. compiles the package with the current CellScript compiler;
3. checks the canonical manifest, Edition 2026, and resolved-profile identity;
4. records the build evidence; and
5. promotes the release to `verified_build` only after those checks succeed.

Pending and rejected entries are not shown by default search. They remain
available through direct audit URLs or explicit status filters.

The Registry status names are deliberately narrower than a general security
claim:

| Status | Meaning |
| --- | --- |
| `source_published` | The signed source record and snapshot were admitted. Compiler verification has not completed. |
| `indexed_pending` | Registry indexing or verification publication is still pending. |
| `verified_build` | The recorded compiler/build checks passed for the bound source and profile. This is not a contract audit. |
| `deployed` | Separate deployment evidence was accepted and checked against its immutable identity. |
| `on_chain_committed` | The configured chain-evidence path observed the required sufficiently confirmed live commitment. |

Generic administrative status changes cannot manufacture
`verified_build`, `deployed`, or `on_chain_committed`. Those transitions use
the ordered evidence-promotion path and bind each new state to its preceding
evidence.

## Installing A Registry Source Package

Install an accepted CellScript source package with:

```bash
cellc install namespace/package@version
```

`cellc install` and `cellc update` use the public API's accepted-status view by
default. Before placing the dependency in the local package graph, the CLI
checks:

- the immutable snapshot descriptor SHA-256;
- archive and path safety;
- each file's BLAKE2b hash;
- the whole source-tree hash;
- the `Cell.toml` package identity;
- Edition 2026; and
- the resolved compatibility-profile identity.

An explicit install of an unverified or quarantined release requires the
corresponding acknowledgement. That choice is stored with the dependency so a
later lock refresh or build does not silently forget the risk decision.

The older `CELLSCRIPT_REGISTRY_URL` Git/`registry.json` path remains available
as an explicit offline override. It is no longer the default public authority.

## Registry Artifacts And Chain Evidence

The Registry is not limited to CellScript dependency packages. A publication
declares its artifact kind, source language, profile, and consumption mode.
CellScript source packages, CKB executables, runtime verifiers, reproducible
binaries, and copy-only templates can all be discovered, but they are not
consumed in the same way:

- only a CellScript source package with dependency consumption enters
  `cellc` package resolution;
- a deployable executable is verified, pinned, deployed, and referenced as a
  CellDep through explicit artifact commands;
- a runtime verifier is recorded as part of the trusted computing base; and
- a template is copied without becoming an implicit package dependency.

For a reproducible-binary profile, a manifest flag is not enough to claim
reproducibility. The Registry requires signed reports from between two and
sixteen distinct builders, subject to the configured builder and trust-domain
policy. Reports bind the environment, source, build recipe, executable, build
log, builder identity, and preceding evidence.

Mainnet deployment and Registry commitment support is implemented in the 0.23
tree, including RPC liveness checks and wallet transaction intents. At the
release boundary, the production commitment path remains disabled because the
canonical Registry Type Script, custody Lock, and required code CellDeps have
not all been deployed and configured. The public service being live is not
evidence of an on-chain mainnet commitment.

## Production And Pudge Environments

Production accepts mainnet authorisation and deployment evidence only. Pudge
testing runs through a separate Registry Sandbox with its own API, database,
object storage, signing origin, wallet state, and testnet evidence.

Sandbox releases leave discovery 72 hours after admission. Their version JSON
is removed at expiry, and their source objects are removed after a further
24-hour grace period. This cleanup removes Registry indexing and off-chain
objects; it does not erase Cells or history from the Pudge chain.

The production site does not expose a testnet selector. This prevents a testnet
record or wallet state from being presented as production evidence.

At the 2026-08-11 release snapshot, the production readiness endpoint reported
`registry_environment = production`, `ckb_network = mainnet`, and
`registry_commitment = disabled`. The Pudge endpoint reported
`registry_environment = testnet-sandbox`, `ckb_network = testnet`, and
`registry_commitment = configured_and_live`.

The Registry service, compiler worker, and browser-session implementation were
deployed and regression-tested. A publisher-owned clean-machine production
publish and first consumer install remained the explicit adoption checkpoint
at release time.

## CLI, LSP, WASM, And Website Changes

Edition 2026 is carried through the same compiler path in package commands,
the LSP, and native APIs. The WASM metadata API requires an explicit edition
argument and accepts only `"2026"` in this release.

The browser WASM build remains metadata-only. It does not emit a CKB ELF.

Registry list and detail pages read the live production API and display the
edition separately from the compatibility-profile hash. If the API is
unavailable, the website can show the checked-in fixture only as a labelled,
read-only mirror. The old “Coming Soon” Registry page is gone.

Submission now records artifact kind and source language independently. A Rust
CKB executable, a CellScript dependency, a runtime verifier, and a copy-only
template no longer appear to be interchangeable package types in the UI.

## Playground Experience Upgrade

The website rollout accompanying 0.23 also turns the browser Playground into a
more reliable, Cell-oriented workbench:

- browser-local workspaces preserve source files, the selected entry, active
  panels, and saved or unsaved state across refreshes;
- a compile error keeps the last successful output visible and clearly marks
  it as stale;
- a failed compiler Worker can be restarted without reloading the page;
- Cell Flow provides a visual view of actions and Cell transitions; and
- Inspector connects actions, types, diagnostics, and metadata while keeping
  the raw compiler output available.

These changes make it easier to experiment, inspect compiler decisions, and
recover from mistakes without losing context. The browser boundary remains
intentionally narrow: the Playground uses CellScript's metadata-only WASM
compiler path and does not generate deployable CKB ELF artifacts.

## Native Tooling And Source Policy

The 0.23 line removes Python from active project tooling. The Rust
`cellscript-tools` crate now owns the gate, evidence, fixture, NovaSeal,
Evolving-DOB, and CKB acceptance paths. Website data generation stays in
tracked Node modules.

Every gate runs a source-policy check across the repository and initialized
submodules. It rejects retired interpreter sources, bytecode caches, captured
traceback logs, and active tooling references to the removed path.

Native fixture generation reads live reports only from an explicit evidence
root. A clean checkout therefore cannot pass by accidentally finding stale
reports under a developer's previous `target/` directory.

This tooling rewrite does not strengthen the meaning of the underlying
evidence. iCKB equivalence, NovaSeal pinning, stateful CKB transactions, and
website/WASM checks keep their separate evidence boundaries.

## Syntax And Example Cleanup

0.23 does not redesign actions, `verification` blocks, invariants, destruction
policies, parameter sources, or Registry namespaces. The syntax audit instead
closed several consistency gaps in checked-in examples and fixtures:

- canonical type declarations use comma-terminated fields;
- the parser still accepts comma-free fields as compatibility input, and the
  syntax-combination matrix tests both forms;
- atomic-swap, NFT, timelock, and multi-phase-DAO examples use a named
  `U64_MAX` value for overflow guards instead of repeating raw boundary
  literals; and
- crypto-primitive CKB-VM fixtures now place `CSARGv1` in
  `WitnessArgs.input_type` rather than using the removed raw-witness alias.

The formatter, bundled examples, syntax-combination audit, and CKB-VM fixtures
now agree on the canonical forms.

## Scope And Evidence Boundaries

The following statements remain outside the 0.23 release claim:

- witness bytes are not authority without signature verification and key
  binding;
- `WitnessArgs.input_type` is not the Type Script of an input Cell;
- successful compilation does not prove that a transaction can be funded,
  built, dry-run, admitted to the tx pool, committed, or kept live;
- `verified_build` is not a security audit or semantic-equivalence proof;
- the Pudge Sandbox does not provide mainnet evidence;
- the bounded Fiber and RGB++ work is not a complete production compatibility
  matrix;
- the proposed Off-Chain Session Runtime profile is not part of 0.23; and
- unaudited CellScript contracts are not recommended for mainnet deployment.

## Validation

Repository contributors use the unified gate entry point:

```bash
# Local development checks
./scripts/cellscript_gate.sh dev

# Pull-request and merge-readiness checks
./scripts/cellscript_gate.sh ci

# ABI, code generation, RISC-V, and stateful backend checks
./scripts/cellscript_gate.sh backend

# Clean-source, pinned-CKB, exact-artifact, and release evidence
./scripts/cellscript_gate.sh release --ckb-repo /path/to/pinned/ckb
```

`dev` and `ci` are development and merge gates. They do not create a stable
release or production CKB claim. `backend` covers the stricter generated-code
boundary. `release` adds the external dependencies and evidence required by the
release process. A lighter gate passing does not imply that a heavier gate
passed.

The public Registry endpoints can be checked separately:

```bash
curl --fail --silent --show-error https://api.registry.cellscript.dev/ready
curl --fail --silent --show-error 'https://api.registry.cellscript.dev/v1/artifacts?limit=5'
curl --fail --silent --show-error https://registry.cellscript.dev/health
curl --fail --silent --show-error https://api.testnet.registry.cellscript.dev/ready
```

These calls show service configuration and liveness. They do not establish
package security, publisher identity, or chain commitment.

## Further Documentation

- [CellScript Edition Policy](https://github.com/CellScript-Labs/CellScript/blob/v0.23.0/docs/CELLSCRIPT_EDITION_POLICY.md)
- [Entry Witness ABI](https://github.com/CellScript-Labs/CellScript/blob/v0.23.0/docs/CELLSCRIPT_ENTRY_WITNESS_ABI.md)
- [Registry end-to-end tutorial](https://github.com/CellScript-Labs/CellScript/blob/v0.23.0/docs/wiki/Tutorial-12-Phase1-Registry-End-to-End.md)
- [Package provenance and deployment identity](https://github.com/CellScript-Labs/CellScript/blob/v0.23.0/docs/CELLSCRIPT_PACKAGE_PROVENANCE_AND_DEPLOYMENT_IDENTITY.md)
- [CKB target profiles](https://github.com/CellScript-Labs/CellScript/blob/v0.23.0/docs/wiki/Tutorial-05-CKB-Target-Profiles.md)
- [Metadata verification and production gates](https://github.com/CellScript-Labs/CellScript/blob/v0.23.0/docs/wiki/Tutorial-06-Metadata-Verification-and-Production-Gates.md)
- [0.23 roadmap](https://github.com/CellScript-Labs/CellScript/blob/v0.23.0/roadmap/CELLSCRIPT_0_23_ROADMAP.md)
- [Changelog](https://github.com/CellScript-Labs/CellScript/blob/v0.23.0/CHANGELOG.md)
