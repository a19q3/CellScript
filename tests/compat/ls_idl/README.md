# Pinned upstream LS-IDL compatibility fixtures

These fixtures preserve the current public LS-IDL inputs used by the projects
linked from the Nervos Talk proposal. They are test evidence, not a fork of the
protocol and not an endorsement of the example Lock Scripts.

Pinned repositories:

- [`OWK50GA/ckb-idl-derive`](https://github.com/OWK50GA/ckb-idl-derive) at
  `e7ee35766b9084099e9d840ccd37d2b5d40074a1`;
- [`OWK50GA/ckb-idl-client`](https://github.com/OWK50GA/ckb-idl-client) at
  `7d883e0abccba56d423449b673567ee817747936`; and
- [`OWK50GA/ckb_sudt_script`](https://github.com/OWK50GA/ckb_sudt_script) at
  `c20ce3f4813100b78076fd447a0234bb5ad46bbb`.

Raw-byte SHA-256 pins:

| Fixture | SHA-256 |
| --- | --- |
| `ckb-idl-client/test-vectors.json` | `a9a6dca4fd0c5fcd2ca7aea6468784be7fdb29d6274049f07090cbab0ce9c1bb` |
| derive `multisig-2of2-nonce/idl.json` | `587098bbe12e37a7394d06ff711a59242f033759e9ba7f5b62b8f6a234275063` |
| derive `pow-lock/idl.json` | `d551803734459f28b2849f13b2111778d3753b518701a86a434e9438df86e2d6` |
| derive `schnorr-pubkey-recovery/idl.json` | `b37329b5fb13b25de94ef068724839f356096bc3516dda461b516ee983a8d371` |
| derive `secp256k1-timelock/idl.json` | `056bc4f2b11bc7f0dfead9f2dcc0ec5097b42b353d4577b3836ef872b121710f` |
| derive `simple-lock/idl.json` | `d28abead992546908eb483c24667e58302f193c00e08f6cbed1a6302995ca1c0` |
| script `simple-lock/idl.json` | `6fd2ab0171167c6862582c4e95a6de7b1cd153f77a936af7e52be6599ddddd31` |
| script `timelock-lock/idl.json` | `18ae57828b5fbd0c8df0900eed1153e7585587d4049900c50729616227a9beda` |

The three upstream files without a final newline are stored as Base64 so Git
and patch tooling cannot silently change the bytes under test. The Rust test
decodes them before hashing or validating them.

`tests/ls_idl_upstream.rs` admits every current upstream IDL document, pins all
17 client vectors, covers all seven current wire types, and confirms that the
one unknown-type vector still fails closed at Registry schema admission. The
separate `scripts/cellscript_ls_idl_upstream_acceptance.sh` test uses clean
checkouts at these commits and runs the actual upstream Rust client against
the Registry compatibility handler. It then creates a disposable worktree,
builds all three example contracts from the unmodified merged upstream source,
binds the two Lock Script ELFs to their exact IDL bytes, and runs all 25
upstream CKB-VM tests against the bound artifacts.

[Upstream PR #7](https://github.com/OWK50GA/ckb_sudt_script/pull/7) merged the
two runtime fixes first identified by this acceptance work: the contract
Makefiles enable CKB's `lower-atomic` LLVM pass, and the timelock reads the
transaction `HeaderDep` included by its tests. CellScript no longer applies a
compatibility overlay for these paths.

This evidence establishes schema compatibility, exact-byte preservation, the
SHA-256 suffix contract, and local CKB-VM execution of the unmodified upstream
examples.
It does not establish signature correctness, production transaction validity,
or a security audit.
