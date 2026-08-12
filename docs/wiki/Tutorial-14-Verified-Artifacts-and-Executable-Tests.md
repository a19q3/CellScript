# Tutorial 14: Verified Artifacts and Executable Tests

CellScript 0.24 added two related boundaries: a standalone checker for generated
CKB ELF bundles, and package scenarios that must name and run an execution
backend. Together they make more compiler claims independently inspectable
without calling local execution chain evidence.

The 0.25 line extends the same four-file bundle with
`cellscript-typed-semantics-v1` inside lowering record v2. The checker now
recomputes the typed record and its connection to entry ABI and final machine
blocks; failures use stable `V2419` and `V2420` codes. It still keeps
`semantic_equivalence_claimed = false`.

## Build the Four-File Bundle

The checked-in `scenario_basics` package is the smallest complete example. From
the repository root:

```bash
cd examples/scenario_basics
cellc build --frozen --offline --json
```

The build emits:

```text
build/main.elf
build/main.elf.meta.json
build/main.elf.lowering.json
build/main.elf.sourcemap.json
```

The lowering record is a canonical, versioned boundary over typed types,
locals, operations, calls, effects, ownership and borrows; entries, basic
blocks, CFG and call edges; ABI and stack declarations; ProofPlan links,
syscalls, runtime-error exits, and final machine ranges. The source map connects
source spans and lowering blocks to those final instruction ranges. All four
files bind the same source, resolved compatibility profile, and artifact.

Assembly output does not emit or claim this boundary.

## Run the Independent Checker

Verify the default bundle:

```bash
cellc verify-artifact build/main.elf --json
```

For non-default paths:

```bash
cellc verify-artifact build/main.elf \
  --metadata evidence/main.meta.json \
  --lowering-record evidence/main.lowering.json \
  --source-map evidence/main.sourcemap.json \
  --json
```

The standalone checker validates bounded schema, identity, CFG, ABI, frame,
stack, ProofPlan, ELF, RV64 instruction, branch/call, syscall, block-digest,
and source-map invariants. It does not load the CellScript front end or code
generator. Keep these report fields distinct:

- `binding_verification`: the bundle identities agree;
- `structural_verification`: the independent structural policy passed;
- `lowering_record_verification`: the lowering contract passed;
- `ckb_vm_evidence`: whether CKB-VM was actually executed;
- `chain_evidence`: whether separate chain evidence was supplied; and
- `semantic_equivalence_claimed`: remains `false` for this boundary.

A successful checker result is not proof that arbitrary source is equivalent
to arbitrary RISC-V. It is also not RPC admission, deployment, commitment, or
confirmation evidence.

## Add an Executable Scenario

Place a `*.scenario.json` file under the package's `tests/` directory. The v1
schema names the confined source file, CKB target profile, entry, initial live
Cells, ordered replacement steps, dependencies, headers, `since`, witnesses,
limits, and an exact expectation.

See `examples/scenario_basics/tests/pass.scenario.json` and
`assertion-failure.scenario.json` for runnable positive and exact-negative
fixtures. Scenario sources intentionally stay in the same `tests/` directory:
v1 rejects absolute paths and parent traversal instead of letting a fixture
escape its evidence root.

A minimal positive shape is:

```json
{
  "schema": "cellscript-test-scenario-v1",
  "name": "main-succeeds",
  "source": "main.cell",
  "target_profile": "ckb",
  "entry": { "kind": "action", "name": "main", "args": [] },
  "initial_cells": [],
  "steps": [{
    "name": "run-main",
    "consumes": [],
    "outputs": [],
    "cell_deps": [],
    "header_deps": [],
    "since": {},
    "witnesses": [],
    "expectation": { "status": "pass", "result": "()", "runtime_error": null }
  }],
  "limits": {
    "max_steps": 1000,
    "max_cycles": 10000000,
    "max_transaction_bytes": 65536,
    "minimum_cell_capacity": 100000000
  },
  "oracle": null
}
```

Negative scenarios use `status = "runtime-error"` and must match both the
registered numeric `CellScriptRuntimeError` and its stable name. Unknown fields,
path escape, duplicate or stale Cell names, ambiguous indexes, invalid scripts,
and unsupported evidence requests fail before execution.

## Run Both Evidence Tiers

```bash
cellc test --backend simulator --frozen --offline
cellc test --backend ckb-vm --frozen --offline
cellc test --backend all --frozen --offline --json
```

The simulator is deterministic development feedback and is labelled
`development-non-consensus`. CKB-VM execution is labelled
`authoritative-runtime`. `cellc test` cannot report executed success without a
backend and an executable scenario; `--no-run` is the explicit compile-only
escape hatch.

The v1 runner validates multi-step live-Cell bookkeeping: consumed names become
dead, declared outputs become live, and `prior_output` must name a Cell consumed
by the same step. The current CKB-VM backend executes no-argument ELF entries.
It does not yet inject scenario Cells into CKB syscalls. Transaction-shaped
entries must point at the separate stateful CKB oracle and must not be relabelled
as v1 CKB-VM scenario coverage.

## Read Coverage Conservatively

The JSON report binds the compiler, artifact, compatibility profile, checker
policy, lowering record, source map, backend, and evidence tier. Coverage lists
declared and observed entries, lowering blocks, ProofPlan links, runtime errors,
syscalls, and source-linked instruction ranges.

Only the observed entry and exact runtime outcome are promoted. The presence of
an unexecuted branch, ProofPlan obligation, or syscall in metadata is not test
coverage.

## Registry and Production Boundaries

A generic CKB Registry bundle with `source`, `executable`, and `abi` remains
`hash_bound`. CellScript structural admission is opt-in and requires the
complete `metadata`, `lowering_record`, and `source_map` set; partial verified
sidecars fail closed. The least-privilege Registry worker records the checker
version, policy, and report hash as `structurally_verified` evidence without
loading the compiler.

Neither structural Registry admission nor local scenarios replace builder,
dry-run, deployment, commitment, or confirmation evidence. Use the full release
gate only when making a production CKB claim.

## Next

Use [Metadata Verification and Production Gates](Tutorial-06-Metadata-Verification-and-Production-Gates.md)
to place these results in the full evidence ladder, and
[Packages and CLI Workflow](Tutorial-04-Packages-and-CLI-Workflow.md) for the
complete package lifecycle.
