# CellScript 0.26 Release Notes

**Status**: active 0.26 implementation record for `nightly-0.26`.

0.26 turns the narrow fixed-width bounded Cell lifecycle shape from a 0.25
fail-closed placeholder into an executable CKB Type Script contract. This is
not generic Cell collection support: source selection, encoding, ordering,
identity, and resource bounds are deliberately closed.

## Dynamic Group Input Consumption

`input cells: BoundedCellSet<Resource, N>` plus `consume_each` is executable
when `Resource` has a fixed encoded width of 1–512 bytes and
`1 <= N <= 1024`. The generated Script:

- scans relative `GroupInput` indexes for the current Type Script;
- accepts only `CKB_INDEX_OUT_OF_BOUND` as the end of the group;
- probes index `N`, rejecting an `N + 1` member with runtime error 21;
- requires exact Cell data size and the current Type Script hash;
- rejects Lock-Script role confusion;
- executes every predicate exactly once for every decoded element; and
- permits only mutable outer numeric `+=` accumulators as loop state.

The runtime and metadata contract name is
`bounded-type-group-inputs-v1`. Zero cardinality is valid only when another
member, normally a group output, causes the Type Script to execute.

## Versioned Output Plans

`witness plans: BoundedList<Plan, N>` plus `create_each` is executable when the
plan and output resource are fixed-width, the complete create template and
output lock are explicit, the resource declares a non-zero capacity floor, and
the output uses no custom identity policy. The inner plan encoding is:

```text
"CSBPLv1\0" || u32_count_le || fixed_width_plan_elements
```

The maximum inner payload is 4084 bytes, leaving room for the eight-byte
`CSARGv1\0` header and four-byte dynamic argument length in the 4096-byte entry
buffer. Plan element `i` verifies relative `GroupOutput[i]`. The Script checks
complete data, exact lock, Type-only role, capacity floor, per-element
predicates, and a final out-of-bounds probe proving output count equals plan
count. The public `encode_bounded_output_plan_v1` helper constructs the inner
payload; normal entry-witness and CKB-adapter placement APIs wrap it in
`WitnessArgs.input_type` before signing.

The runtime and metadata contract name is `bounded-output-plan-v1`.

## Checked Business Examples

The language example suite includes four production-policy-closed contracts:

- `batches/batch_claim.cell`: non-zero variable-cardinality claims with count and
  amount conservation;
- `batches/atomic_order_settlement.cell`: one through sixteen orders settled in
  one transaction;
- `batches/cell_merge.cell`: two through 128 fragmented Cells merged into exactly
  one amount-conserving output; and
- `batches/bridge_rollup_batch.cell`: bounded messages and receipts with
  canonical consecutive nonces, count equality, and amount conservation.

The examples themselves run in CKB-VM. Adversarial vectors reject claim amount
mismatches, a seventeenth order, merge inflation, and non-consecutive bridge
nonces. Shared runtime vectors additionally cover zero/one/N/N+1 cardinality,
malformed plan magic/length/trailing bytes, exact data size, predicate failure,
missing and extra outputs, lock substitution, and capacity underflow.

## Deliberate Fail-Closed Boundary

0.26 does not promote dynamic or recursive plan/resource layouts,
transaction-wide or Lock Script scans, custom output identities, incomplete
create templates, implicit locks or capacity policy, or arbitrary body
mutation. Those shapes keep the registered runtime error 24 fallback and are
rejected before ASM/ELF generation with E2105 under `--production` or
`--deny-fail-closed`.

## Versioned Evidence

This line advances compile metadata to schema 62 and constraints metadata to
schema 3. The independently checked boundary is
`cellscript-verified-lowering-record-v4` with
`cellscript-typed-semantics-v3`, including dedicated bounded Cell load, plan
load, output verification, and output-end operations.

Merge readiness requires:

```bash
./scripts/cellscript_gate.sh backend
./scripts/cellscript_gate.sh dev
./scripts/cellscript_gate.sh ci
```
