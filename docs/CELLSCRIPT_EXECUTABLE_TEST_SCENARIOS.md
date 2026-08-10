# CellScript Executable Test Scenarios

**Status**: implemented on the 0.24 development line

**Scenario schema**: `cellscript-test-scenario-v1`

**Report schema**: `cellscript-test-report-v1`

## Running Tests

`cellc test` no longer treats compile-only discovery as executed tests. Unless
`--no-run` is selected, a backend and at least one `*.scenario.json` fixture
are required:

```bash
cellc test --backend simulator
cellc test --backend ckb-vm
cellc test --backend all --json
cellc test --no-run
```

The two execution tiers are deliberately different:

- `simulator` runs the typed development interpreter and reports
  `development-non-consensus` evidence;
- `ckb-vm` loads the emitted ELF into the maintained CKB-VM runner and reports
  `authoritative-runtime` evidence.

Neither tier is an RPC admission, transaction commitment, or confirmation
claim.

## Scenario Shape

A scenario sits beside its confined relative `.cell` source and declares:

- an action or lock entry and typed scalar arguments;
- named initial live Cells with capacity, data, lock, and optional Type Script;
- ordered steps with consumed Cells and named replacement outputs;
- CellDeps, header deps, per-input `since`, and `WitnessArgs` lock,
  `input_type`, and `output_type` fields;
- either `pass` or one exact registered runtime error code and name;
- maximum interpreter steps, CKB-VM cycles, serialized fixture bytes, and a
  minimum Cell capacity; and
- an optional reference to the separate stateful CKB oracle.

All security-sensitive structs reject unknown fields. Source and oracle paths
must be relative and path-confined. Hashes, scripts, indexes, witnesses,
duplicate names, stale/dead Cell references, output-name reuse, and limits are
validated before execution.

See `tests/scenarios/positive.scenario.json` for a two-step replacement and
`tests/scenarios/assertion-failure.scenario.json` for an exact
`assertion-failed` (`5`) expectation.

## Multi-Step State Boundary

The v1 runner maintains a local live-Cell set. Consumed names become dead,
outputs become live, and `prior_output` must name a Cell consumed by the same
step. This catches stale references, double consumption, and ambiguous
replacement chains.

The local state model validates scenario bookkeeping. It does not inject those
Cells into CKB syscalls. The current CKB-VM backend executes no-argument ELF
entries; entries that require transaction syscalls or arguments must reference
the separate stateful oracle and remain outside this v1 runner until a
transaction syscall harness is promoted.

## Exact Failures And Coverage

The runner validates both the numeric `CellScriptRuntimeError` and its stable
name. An unregistered code, a name/code mismatch, success where failure was
expected, or a different runtime error fails the scenario.

Every report binds compiler version, artifact hash, compatibility profile,
checker name/version/policy, lowering-record hash, source-map hash, backend,
and evidence tier. Coverage reports list declared and observed entries,
lowering blocks, ProofPlan links, runtime errors, syscalls, and source-linked
instruction ranges.

Coverage is conservative: v1 claims only the observed entry and exact runtime
outcome. It does not claim unexecuted branches, ProofPlan obligations, or
syscall sites merely because they were present in compiler metadata.

## Gate Coverage

- `dev` runs the simulator scenarios.
- `ci` and `backend` run both simulator and CKB-VM scenarios.
- The existing stateful CKB harness remains the transaction-shaped oracle and
  is not replaced by local scenario bookkeeping.
