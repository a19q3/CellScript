# CellScript Public Interfaces And Compatibility

**Status**: implemented on `nightly-0.25`

**Schemas**: `cellscript-package-interface-v1` and
`cellscript-interface-compatibility-v1`

## What The Interface Represents

Every successful compile now constructs a canonical public package interface.
It is stored in compile metadata as `public_interface`; its CKB BLAKE2b-256
identity is stored as `interface_hash`.

The interface contains only exported items and records the contracts that a
consumer or Registry upgrade must preserve:

- canonical module, type, constant, action, lock, and function identities;
- explicit `public`, `public(package)`, and `private` visibility;
- type parameters, phantom parameters, value abilities, and Cell lifecycle
  capabilities as separate fields;
- fields, enum variants, fixed layouts, and type identities;
- callable parameters, source qualifiers, outputs, return types, effects, and
  entry witness ABI;
- concrete generic instantiations;
- target, VM, witness, lock-args, source-encoding, Spawn/IPC, and compatibility
  profile identities; and
- generated-builder and deployment-contract hashes.

Edition 2026 keeps the historical public-by-default behavior for an item with
no modifier. New reusable packages should spell visibility explicitly so a
future edition migration does not silently change the exported surface.

```cellscript
module example::math

public struct Pair<T: copy + drop + store + fixed + serializable + non_linear>
    has copy, drop, store, fixed, serializable, non_linear {
    left: T
    right: T
}

public(package) fn sum_pair(pair: Pair<u64>) -> u64 {
    return pair.left + pair.right
}

private fn implementation_detail(value: u64) -> u64 {
    return value
}
```

`public(package)` items are visible to modules in the same package but are not emitted
as dependency-facing exports. `private` items are local to their module.

## Emit An Interface

For a source file or package:

```bash
cellc interface path/to/package --json
cellc interface path/to/package --output target/package.interface.json
```

The JSON envelope contains both `interface` and `interface_hash`. The hash is
computed over canonical JSON; reordering source declarations does not change
the identity after the compiler's canonical sort.

## Compare Two Releases

```bash
cellc interface-diff \
  --old target/old.interface.json \
  --new path/to/candidate-package \
  --json
```

The report classifies changes across six independent dimensions:

| Dimension | Examples of breaking changes |
| --- | --- |
| `source_api` | removing an export; changing a type parameter, parameter, return type, or output |
| `serialized_layout` | changing fields, variants, offsets, fixed sizes, or type identity |
| `runtime_abi` | changing an entry ABI, witness placement, target, or versioned VM contract |
| `effects_capabilities` | changing callable effects, value abilities, or Cell lifecycle capabilities |
| `builder` | changing the generated transaction-builder contract |
| `deployment` | changing the deployment/runtime identity contract |

A breaking report exits with stable compiler code `E2501`. Additive exports are
reported as compatible changes; they still change `interface_hash`, so a
consumer can choose whether it accepts a new exact identity.

## Registry Admission

CellScript source publication includes the canonical interface and hash in the
signed publish payload. The Registry API recomputes the hash, rejects a
mismatch, and compares a candidate release with the latest admitted interface.
An incompatible upgrade is rejected before release admission. The standalone
Registry verifier also checks that the stored interface and `interface_hash`
agree.

This is package compatibility evidence, not proof that an artifact is safe or
deployed. Registry verification, the typed semantic checker, CKB-VM execution,
deployment evidence, and chain commitment remain separate states.

## Typed Semantics Relationship

The public interface answers “what can a dependency rely on?” The
`cellscript-typed-semantics-v1` record answers “what typed operations and
control-flow facts were lowered?” Both hashes are bound into metadata. ELF
builds additionally bind the typed record to the verified lowering and machine
records described in
[CellScript Verified Artifact Boundary](CELLSCRIPT_VERIFIED_ARTIFACT_BOUNDARY.md).
