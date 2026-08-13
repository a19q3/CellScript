# LS-IDL Registry Profile

**Status**: CellScript 0.24 protocol and tooling contract

**Profile schema**: `cellscript-registry-ls-idl-interface-v1`

**LS-IDL format version**: `0.1`

This profile lets the CellScript Artifact Registry publish and resolve an
LS-IDL witness interface for a deployed CKB Lock Script. It preserves the
upstream commitment rule exactly:

```text
code Cell data = executable bytes || SHA-256(raw idl.json bytes)
```

The Registry stores the original IDL object bytes in the immutable release
bundle. It never parses and reserialises those bytes on the read path.

## What The Profile Proves

An accepted profile proves all of the following:

- the ABI object is valid JSON within the Registry's size and field budgets;
- the document uses the supported LS-IDL 0.1 schema and witness types;
- the declared digest equals `SHA-256` of the exact ABI object bytes;
- the executable object's final 32 bytes equal that digest;
- the package is a deployable `ckb_executable` with `script_role = "lock"`;
- a lookup result belongs to an active, public, chain-verified deployment on
  the requested Registry network.

It does **not** prove that the Lock Script decodes witness data as described,
that every described field is semantically enforced, that a transaction is
valid, or that the Script is secure. Build verification and security review
remain separate evidence.

## Artifact Profile Contract

`profile_contract.interface` is accepted only on deployable CKB executables:

```json
{
  "schema": "cellscript-registry-ls-idl-interface-v1",
  "format": "ls-idl",
  "format_version": "0.1",
  "object_role": "abi",
  "content_type": "application/vnd.ckb.ls-idl+json",
  "encoding": "linear-le-v0",
  "commitment": {
    "algorithm": "sha256",
    "placement": "code-cell-data-suffix-32",
    "digest": "<64 lowercase hexadecimal characters>"
  }
}
```

Unknown keys, alternate algorithms, alternate commitment placements, missing
ABI objects, digest mismatches, and executable-suffix mismatches fail closed.
The artifact bundle continues to use CKB Blake2b-256 for its executable
`artifact_hash`; LS-IDL's ABI commitment remains SHA-256. These hashes have
different roles and are not interchangeable.

## Accepted IDL Document

The document may contain only these top-level fields:

- `idl_version` (optional string, matching upstream clients that default it);
- `name` (optional string, matching derive output that may omit it);
- `witness` (required array, at most 256 fields);
- `description` (optional string);
- `script_version` (optional string); and
- `signing` (optional object with non-empty string fields `algorithm`,
  `message`, and `hasher`).

Each witness field contains only `name`, `type`, `required`, and
`description`. Names must be unique. The supported types are:

| Type | Encoding |
| --- | --- |
| `uint8` | one unsigned byte |
| `uint32` | four-byte little-endian unsigned integer |
| `uint64` | eight-byte little-endian unsigned integer |
| `secp256k1_sig` | 65 bytes |
| `secp256k1_pubkey` | 33 bytes |
| `schnorr_sig` | 64 bytes |
| `bytes` | four-byte little-endian length followed by that many bytes |

The current linear decoder treats `required` as interface metadata. It does
not introduce a presence bitmap or conditional field skipping, so consumers
must not interpret `required: false` as a wire-level omission rule.

## Public Read API

The canonical lookup is:

```text
GET /v1/ckb/scripts/:code_hash/interfaces/ls-idl
    ?network=mainnet|testnet
    &hash_type=data|data1|data2|type
    [&data_hash=0x...]
```

`data_hash` is mandatory for `hash_type=type`, where a type hash alone may not
uniquely identify executable data. More than one matching deployment returns
`409` instead of choosing arbitrarily.

The compatibility route is:

```text
GET /idl/:code_hash
```

It is retained for existing LS-IDL clients and returns the same original
bytes. New integrations should use the canonical route so network, hash type,
and data-hash identity are explicit.

Successful responses use
`application/vnd.ckb.ls-idl+json` and expose:

- `ETag`;
- `x-ls-idl-format-version`;
- `x-ls-idl-sha256`;
- `x-ls-idl-coordinate`;
- `x-ls-idl-commitment`; and
- `x-ls-idl-verification`.

Clients must hash the response body directly. JSON-equivalent reformatting is
not byte-equivalent and therefore does not preserve the commitment.

## CLI Workflow

Validate a document and optionally its existing executable binding:

```bash
cellc artifact ls-idl validate --idl idl.json
cellc artifact ls-idl validate --idl idl.json --executable lock
```

Append the raw-byte digest to an executable without silently overwriting it:

```bash
cellc artifact ls-idl bind \
  --idl idl.json \
  --executable lock \
  --output lock.ls-idl
```

Generate a publish-ready artifact bundle and manifest:

```bash
cellc artifact ls-idl bundle \
  --idl idl.json \
  --executable lock.ls-idl \
  --source lock.rs \
  --namespace example \
  --name example-lock \
  --release 0.1.0 \
  --language rust \
  --hash-type data1 \
  --dep-type code \
  --toolchain rust-1.97.1 \
  --source-revision <40-hex-git-commit> \
  --output artifact.bundle.json \
  --artifact-manifest-output Artifact.toml
cellc publish --artifact-manifest Artifact.toml --dry-run --json
```

Fetch exact bytes by deployed Script identity:

```bash
cellc artifact ls-idl fetch \
  --code-hash 0x<64-hex> \
  --hash-type data1 \
  --network mainnet \
  --output idl.json
```

The VS Code extension exposes the validate, bind, and fetch operations through
the command palette. The Registry website exposes both package-bound interface
facts and a direct Script-identity lookup.

## Storage And Admission

Migration `0010_ls_idl_interfaces.sql` adds a partial lookup index over public,
chain-verified deployed evidence. The API narrows candidates in the database,
then rechecks release identity, immutable bundle identity, one-and-only-one ABI
object, raw-byte digest, and profile contract before returning bytes.

The normal compiler-backed Registry worker and the least-privilege
artifact-only verifier both enforce the same profile. The latter has no
CellScript compiler dependency. A publish that merely labels arbitrary JSON as
LS-IDL, supplies a detached digest, or binds a digest to the wrong executable
is rejected before it can become searchable.

## Compatibility Evidence

CellScript's curated vectors live in
`examples/registry_ls_idl/vectors.json`. The complete upstream client vector
corpus is intentionally referenced rather than copied:

- `ckb-idl-derive` commit
  `e7ee35766b9084099e9d840ccd37d2b5d40074a1`;
- `ckb-idl-client` commit
  `7d883e0abccba56d423449b673567ee817747936`;
- upstream `test-vectors.json` SHA-256
  `a9a6dca4fd0c5fcd2ca7aea6468784be7fdb29d6274049f07090cbab0ce9c1bb`.

The upstream mini Registry example parses and reserialises JSON, so it is not
used as the byte-preserving production storage contract. One upstream property
test also constructs a Blake2b commitment while the production client verifies
SHA-256. CellScript follows the production client and proposal commitment,
records those upstream revisions, and tests SHA-256 end to end.

## Operational Boundary

Deploying the website does not deploy the Registry API. Operators must apply
migration `0010_ls_idl_interfaces.sql`, roll the API and verification worker,
and verify the canonical and compatibility routes before advertising live
lookup availability. Existing artifact records without an interface contract
remain valid and are not returned by LS-IDL lookup.
