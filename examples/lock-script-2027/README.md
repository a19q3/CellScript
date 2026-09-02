# Edition 2027 Lock Script preview

This package exercises the experimental `0.26b` native `lock_script` surface.
It keeps the current entry-witness and CKB target ABIs while making the
protected Cell, current Lock Script arguments, and witness provenance explicit.

```bash
cellc expand examples/lock-script-2027 --json
(cd examples/lock-script-2027 && cellc check --target-profile ckb)
```

The generated disposition is `AuthorizationOnly`: the Lock Script proves that
the protected Cell may be spent, while its Type Script or an explicit
transaction policy remains responsible for successor, retirement, data,
identity, Type Script, and capacity constraints. The package deliberately does
not claim that a Lock Script governs those business-level relations.

The grammar is a bounded preview rather than the frozen CellScript 1.0 source
contract. The complete implemented boundary, rejected forms, issue conflicts,
and deferred syntax are documented in
[`docs/CELLSCRIPT_2027_PREVIEW_GRAMMAR.md`](../../docs/CELLSCRIPT_2027_PREVIEW_GRAMMAR.md).
