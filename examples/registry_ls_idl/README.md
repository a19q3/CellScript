# Registry LS-IDL example

This fixture demonstrates the 0.24 Registry profile for an LS-IDL 0.1 CKB
Lock Script interface. `idl.json` is intentionally formatted rather than
canonicalised: its exact bytes, including whitespace and final newline, are
the bytes committed by SHA-256 and returned by the Registry.

`lock.rs` shows the corresponding `ckb-idl-derive` declaration. It is a small
integration example, not an audited or deployable Lock Script.

Prepare a publishable generic artifact from a real RISC-V Lock Script ELF:

```bash
cellc artifact ls-idl validate --idl idl.json
cellc artifact ls-idl bind \
  --idl idl.json \
  --executable build/demo-lock \
  --output build/demo-lock.ls-idl
cellc artifact ls-idl bundle \
  --idl idl.json \
  --executable build/demo-lock.ls-idl \
  --source lock.rs \
  --namespace demo \
  --name ls-idl-lock \
  --release 0.1.0 \
  --language rust \
  --toolchain 'rustc 1.97.1 + ckb-std' \
  --source-revision '<immutable-git-commit>' \
  --output bundle.json \
  --artifact-manifest-output Artifact.toml
cellc publish --artifact-manifest Artifact.toml --dry-run
```

After the Registry has accepted the bundle and chain-verified its deployment,
clients can retrieve the exact IDL bytes:

```bash
cellc artifact ls-idl fetch \
  --code-hash 0x<64-hex> \
  --hash-type data1 \
  --data-hash 0x<64-hex> \
  --output fetched-idl.json
```

The Registry proves bounded schema conformance, immutable object hashes, and
the `SHA-256(idl.json)` executable suffix. It does not prove signature
correctness, authorization semantics, or the security of the Lock Script.
`required = false` remains descriptive in LS-IDL 0.1; it does not make a field
conditionally absent from the current linear decoder.

`vectors.json` is a small Registry-facing compatibility subset. The upstream
client repository remains authoritative for its complete evolving vector set.
This example was checked against `ckb-idl-derive` commit
`e7ee35766b9084099e9d840ccd37d2b5d40074a1` and `ckb-idl-client` commit
`7d883e0abccba56d423449b673567ee817747936`; that client's complete
`test-vectors.json` has SHA-256
`a9a6dca4fd0c5fcd2ca7aea6468784be7fdb29d6274049f07090cbab0ce9c1bb`.
