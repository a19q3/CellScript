# CellScript Verified Artifact Boundary

**Status**: typed boundary implemented on the 0.25 development line

**Schemas**: `cellscript-verified-lowering-record-v3`,
`cellscript-typed-semantics-v2`,
`cellscript-source-artifact-map-v1`, and
`cellscript-artifact-checker-policy-v1`

**Metadata schema**: 61

## Purpose

Every CKB RISC-V ELF build now emits two canonical sidecars in addition to the
artifact and compile metadata:

```text
build/main.elf
build/main.elf.meta.json
build/main.elf.lowering.json
build/main.elf.sourcemap.json
```

The typed semantic record retains checked types, locals, calls, effects,
ownership, borrow regions, concrete generic instantiations, layouts, and CFG
operations in a parser-free schema. Lowering record v3 embeds that record and
binds it to the final machine layout. Every typed block is accounted for;
optimized/elided typed blocks have an explicit empty machine-block list, while
materialized blocks carry exact typed-block hashes. The source map binds source
spans and lowering block IDs to final instruction ranges. All records are hash-bound into
compile metadata and validated immediately after compilation.

The sidecars do not claim complete source-to-machine semantic equivalence.
Their explicit claims are typed-record validation, `binding-verified` for the
lowering record, and `structurally-verified` for machine code. The report keeps
`semantic_equivalence_claimed = false`.

## Independent Checker

`crates/cellscript-artifact-checker` has no production dependency on the
CellScript parser, resolver, type checker, IR, optimizer, assembler, or code
generator. It accepts artifact bytes, compile metadata, one canonical lowering
record, one canonical source map, and explicit policy budgets.

The checker is an independently publishable crate because the published
`cellscript` crate uses it as a production dependency. Release tooling must
publish the exact checker version before the matching compiler version; CI
verifies the same graph offline through an exact local crates.io patch.

The checker independently recomputes and validates:

- schema versions, unknown-field rejection, canonical JSON, counts, ordering,
  uniqueness, and domain-separated hashes;
- entry, block, CFG, reachability, call-depth, recursion, frame, stack-slot,
  typed ABI, capability, and ProofPlan relationships;
- typed semantic schemas, exact constants and operation detail, canonical type
  and local tables, call signatures and effects, definite-definition joins,
  ownership/borrow state transitions, enum/layout hashes, and owner-qualified
  concrete instantiations;
- typed entry/block/operation identities against lowering blocks, final
  machine ABI, and the metadata `typed_semantics_hash`;
- ELF64 little-endian RISC-V identity, exact static sections, read/execute
  segment policy, entry and text/rodata bounds, and absence of dynamic or
  relocation state;
- the bounded RV64 instruction set emitted by CellScript, canonical direct
  calls, aligned branch/call targets, machine terminators, stack-pointer
  adjustments, return-path stack restoration, and declared syscalls;
- every mapped block digest and every source-map range against final ELF bytes;
  and
- compiler, source, profile, artifact, lowering-record, and source-map identity
  agreement.

Declared unreachable machine blocks are not silently treated as reachable.
The record carries a `reachable` bit and the checker recomputes it from every
declared entry.

## Default Budgets

The default v1 policy caps each artifact, lowering record, and source map at
4 MiB; entries at 2,048; blocks and proof records at 65,536; edges at 262,144;
instructions at 1,048,576; call depth at 256; declared stack frames at 1 MiB;
source-map intervals at 65,536; and one diagnostic at 16 KiB. A consumer may
apply a stricter compatible policy.

Budget exhaustion is `V2400`. Input-derived counts are checked before graph
traversal, diagnostic text is bounded, and invalid input must return an error
instead of panicking.

## Stable Rejection Codes

| Code | Boundary |
| --- | --- |
| `V2400` | policy budget exceeded |
| `V2401` | malformed JSON |
| `V2402` | non-canonical JSON |
| `V2403` | unsupported schema or overclaimed verification state |
| `V2404` | non-canonical ordering or duplicate identity |
| `V2405` | referential-integrity failure |
| `V2406` | CFG, reachability, runtime-exit, or terminator failure |
| `V2407` | ABI, frame, stack-slot, or stack-pointer failure |
| `V2408` | ProofPlan coverage failure |
| `V2409` | artifact identity mismatch |
| `V2410` | compile-metadata or compatibility-profile mismatch |
| `V2411` | invalid ELF format |
| `V2412` | invalid or prohibited ELF section/link state |
| `V2413` | instruction outside the checker policy |
| `V2414` | decoded control-flow target or machine terminator mismatch |
| `V2415` | mapped block digest mismatch |
| `V2416` | source-map identity, range, path, or coverage failure |
| `V2417` | syscall declaration or bounded-call contract failure |
| `V2418` | recursion or call-depth policy failure |
| `V2419` | typed semantic schema, type/local/operation, ownership, borrow, layout, instantiation, or effect failure |
| `V2420` | typed semantic hash, lowering-block, entry ABI, call, or final machine binding failure |

The deterministic mutation corpus in `tests/artifact_checker.rs` exercises all
stable rejection codes. It is a regression corpus, not a proof of complete
semantic equivalence.

## CLI Verification

For an ELF build, `verify-artifact` loads the default sidecars automatically:

```bash
cellc verify-artifact build/main.elf --json
```

Use `--lowering-record` and `--source-map` only when the sidecars use custom
paths. The JSON report keeps these states separate:

- `binding_verification`;
- `structural_verification`;
- `lowering_record_verification`;
- `ckb_vm_evidence`;
- `chain_evidence`; and
- `semantic_equivalence_claimed`.

The checker does not execute CKB-VM and does not query a chain. A successful
structural report therefore leaves CKB-VM as `not-executed`, chain evidence as
`not-provided`, and semantic equivalence as `false`.

## Registry Boundary

The Registry preserves generic Rust/C/JavaScript CKB bundles as `hash_bound`
when they provide only `source`, `executable`, and `abi`. A bundle that opts
into CellScript structural verification by including any verified sidecar must
provide all of `metadata`, `lowering_record`, and `source_map`; partial sets
fail closed. Artifact-only admission runs
`cellscript-registry-artifact-verify`, whose normal dependency graph contains
the standalone checker but not the CellScript compiler. A
`structurally_verified` result records checker version, policy schema, and
checker-report hash.

Compiler-backed source-package verification remains a separate worker and a
separate trust state. Structural verification is not a security audit and is
not deployment or chain evidence.

## Compatibility Rules

- Unknown fields and future schema versions fail closed.
- Absolute and parent-traversing source paths are rejected.
- Raw `CSARGv1` witness compatibility is rejected; the compatibility profile
  must use canonical `WitnessArgs.input_type` placement.
- Assembly output has no verified-artifact sidecars and reports the boundary as
  not applicable.
- Consumers must bind all four files from the same build. Mixing a valid ELF,
  metadata file, lowering record, or source map from different builds fails.
