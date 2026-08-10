# CellScript ckb-sdk Builder Cookbook

This crate is a cookbook wrapper around the formal 0.19 adapter crate:

```text
crates/cellscript-ckb-adapter
```

It is intentionally outside compiler core and must not grow a second adapter
implementation.

It demonstrates the boundary:

- CellScript emits an `ActionPlan` through `cellc action build --json`.
- The formal adapter crate materializes a `ResolvedActionTx` with CKB packed
  types.
- `ScriptSpec` constructs arbitrary `ckb_types::packed::Script` values and
  records script hash / args evidence without inventing a second encoding.
- `ScriptRef` reads lock/type scripts back from packed `CellOutput` values so
  adapters can compare live outputs with expected scripts.
- `ScriptCodeDep` binds script code hash / hash type to explicit CellDeps and
  rejects missing or wrong-dep cases before RPC submission.
- `ActionPreview` emits frontend-ready JSON data for consumed inputs, created
  outputs, lineage, witnesses, warnings, and estimated fee without rendering UI.
- `AcceptedActionReport` records cycles, tx-pool acceptance, optional submitted
  tx hash, tx size, occupied capacity, fee, and lineage after node checks.
- `place_entry_witness_payload_before_signing` preserves the existing
  `WitnessArgs.lock` and `output_type` fields while placing the compiler-emitted
  `CSARGv1` payload in canonical `WitnessArgs.input_type`. Call it before any
  lock-script signer because the signature commits to the complete witness.
- `ckb-sdk-rust` owns transaction building, signer integration, RPC cycle
  estimation, tx-pool acceptance, and optional submission.

The placement step is explicit:

```rust
let witness = place_entry_witness_payload_before_signing(
    &base_witness_args,
    EntryWitnessPlacementAbi::WitnessArgsInputTypeV2,
    entry_payload,
)?;
```

`entry_payload` is the raw output of `cellc entry-witness`; the resulting
serialized `WitnessArgs` is the transaction witness. A raw `CSARGv1` payload,
`output_type` alias, or post-signing mutation is not compatible with the 0.23
CKB entry ABI.

The cookbook tests are offline and do not require a running CKB node. Focused
local-node evidence lives in `scripts/cellscript_ckb_adapter_acceptance.sh`.
