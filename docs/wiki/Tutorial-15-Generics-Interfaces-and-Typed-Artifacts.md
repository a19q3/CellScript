# Tutorial 15: Generics, Public Interfaces, and Typed Artifacts

This tutorial covers the 0.25 reusable-value and package boundary. It shows how
to write a bounded generic value, choose visibility, inspect the canonical
interface, compare an upgrade, and check the typed record bound to an ELF.

## 1. Write A Fixed Generic Value

```cellscript
module tutorial::pairs

public struct Pair<T: copy + drop + store + fixed + serializable + non_linear>
    has copy, drop, store, fixed, serializable, non_linear {
    left: T
    right: T
}

public fn swap<T: copy + drop + store + fixed + serializable + non_linear>(pair: Pair<T>) -> Pair<T> {
    return Pair<T> { left: pair.right, right: pair.left }
}

private fn internal_identity(value: u64) -> u64 {
    return value
}
```

CellScript monomorphizes concrete value uses before IR lowering. The compiler
records every instantiation and applies fixed nesting, count, and identity-size
budgets. Ordinary generic containers cannot hide a Cell-backed value.

Value abilities are not Cell authority. `copy`, `drop`, `fixed`,
`serializable`, and `non_linear` describe ordinary values; `create`, `consume`,
`replace`, and other Cell capabilities remain on Cell-backed declarations.

## 2. Use `Option<T>` And Complete Patterns

```cellscript
public fn unwrap_or_zero(value: Option<u64>) -> u64 {
    return match value {
        Option::Some(inner) => { inner }
        Option::None => { 0 }
    }
}
```

Fixed payload enums support recursive tuple, struct, enum, wildcard, and
binding-free or-patterns. Exhaustiveness and linear-value rules are checked
before lowering.

## 3. Use Explicit Loop Control

```cellscript
label outer: for i in 0..10 {
    for j in 0..10 {
        if j == 0 {
            continue
        }
        if i == 5 {
            break outer
        }
    }
}
```

An unlabelled `break` or `continue` targets the nearest loop. A labelled form
must name a visible enclosing `label name: for ...` or `label name: while ...`
loop. The type checker rejects loop control outside a loop, while lowering
records the exact CFG jump checked against the final machine artifact.

## 4. Emit And Compare Interfaces

```bash
cellc interface . --output target/current.interface.json
cellc interface-diff \
  --old target/released.interface.json \
  --new . \
  --json
```

Read the six compatibility dimensions independently: source API, serialized
layout, runtime ABI, effects/capabilities, builder contract, and deployment
contract. A breaking report exits with `E2501`.

## 5. Build And Verify The Typed Artifact

```bash
cellc build --target riscv64-elf --target-profile ckb
cellc verify-artifact build/main.elf --json
```

Metadata schema 60 includes:

- `public_interface` and `interface_hash`;
- `typed_semantics` and `typed_semantics_hash`;
- generic instantiation records; and
- the existing verified lowering and source-map bindings for ELF output.

The independent checker recomputes typed locals, calls, effects, layouts,
control-flow joins, ownership/borrow records, and their machine ABI link. It
uses `V2419` for an invalid typed semantic record and `V2420` for a typed-to-
machine mismatch.

This still does not mean the checker ran CKB-VM or observed a deployment. Keep
compiler, independent-checker, CKB-VM, deployment, commitment, and mainnet
evidence distinct.

## 6. Inspect It In The Playground

The browser compiler remains metadata-only: it emits no ELF. The Playground
now highlights generics, abilities, visibility, bitwise/shift operations, and
loop control, and its project Inspector shows shortened public-interface and
typed-semantics hashes. Use the raw Metadata tab to copy the complete records.
