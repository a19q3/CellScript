# CellScript Collections Support Matrix

**Status**: production boundary document for the current CellScript CKB profile.

CellScript supports dynamic data in several different layers. These layers must
not be collapsed into one generic "collections are supported" claim.

## Support By Layer

| Feature | Schema/ABI | IR construction | Runtime verifier helper | Production status |
|---|---:|---:|---:|---|
| `Vec<u8>` | Yes | Targeted | Targeted create/update-output verification | Supported for documented witness and cell-data paths |
| `String` | Yes | Targeted | Byte-vector verification | Supported as UTF-8 bytes at the schema boundary |
| `Vec<Address>` | Yes | Targeted | Fixed-element vector verification | Supported where metadata marks a Molecule dynamic field |
| `Vec<Hash>` | Yes | Targeted | Fixed-element vector verification | Supported where metadata marks a Molecule dynamic field |
| Fixed byte arrays | Yes | Yes | Exact-size verification | Supported |
| Stack-backed local `Vec<T: FixedWidth>` | Local-only | Yes | Codegen stack-backed lowering | Supported for verifier-local scalar, fixed-byte, and fixed-width named values |
| `Vec<Vec<u8>>` | Boundary | Boundary | No generic helper | Must fail closed unless a concrete lowering is added |
| Generated allocation-backed collection helpers | No | No | Fail-closed entry symbols | Not a production allocator ABI |
| `HashMap<u64, u64>` | Limited | Limited | No production helper surface | Unsupported/fail-closed for production contracts |
| `HashMap<Hash, Token>` | No | No | No | Unsupported; must fail closed |
| `HashSet<T>` | Limited | Limited | No production helper surface | Unsupported/fail-closed for production contracts |
| `BoundedCellSet<Resource, N>` + `consume_each` | Frontend and metadata only | Explicit fail-closed IR; checked predicates retained | No source scan/decoder | Production-rejected with E2105; non-production artifacts return error 24 |
| `BoundedList<Plan, N>` + `create_each` | Frontend and metadata only | Explicit fail-closed IR; predicates and create template retained | No witness codec/output verifier | Production-rejected with E2105; non-production artifacts return error 24 |
| Generic Cell-backed resource collections | No executable ownership model | No | No | Unsupported |

## Stack-Backed Local Vec Rule

The current backend supports bounded local `Vec<T>` operations only when `T`
has a known fixed width and the vector is verifier-local. These operations are
compiler-recognized stack-backed codegen lowering, not calls into a production
allocator ABI. The supported helper surface is:

```text
new, with_capacity, capacity, push, extend_from_slice, len, is_empty,
indexing, first, last, contains, set, remove, pop, insert, reverse, truncate,
swap, clear
```

`Vec::capacity()` reports the fixed stack backing capacity
(`256 / element_width`), not the requested `Vec::with_capacity(n)` argument.
`cellc explain generics` exposes each checked instantiation, including element
type, element width, backing model, helper set, and constructor provenance.

Generated public collection symbols in `src/stdlib/collections.rs` are kept as
fail-closed stubs unless a concrete checked runtime ABI exists. Do not document
or use those symbols as production allocation-backed `Vec`, `HashMap`, or
`HashSet` helpers.

`examples/registry.cell`, `examples/language/registry.cell`, and
`examples/language/order_book.cell` are
compiler/tooling examples for this local helper surface. They are not part of
the bundled CKB production action acceptance matrix.

## Production Rule

Supported dynamic values must have deterministic Molecule metadata and verifier
evidence:

- `molecule_schema_manifest` entry
- dynamic field declaration where applicable
- generated create or update-output verifier marker
- constraints or production-gate evidence for the entrypoint that uses it

Unsupported generic collections must not silently compile into a weaker runtime
shape. They must produce one of:

- compile-time diagnostic
- structured blocker in metadata/constraints
- explicit fail-closed runtime path with a registered runtime error

Every selected consensus-relevant ProofPlan status beginning with `gap:` is a
production blocker, including `gap:runtime-helper-required`,
`gap:builder-evidence-required`, and `gap:metadata-only`. A transaction builder
can supply evidence for construction, but that evidence cannot authorize a
consensus operation the emitted Script does not check.

## Bounded Lifecycle Boundary

The bound `N` is currently a static type and complexity declaration. It is not
evidence that a runtime scan observed at most `N` items. Metadata therefore
records the maximum but leaves actual cardinality unobserved.

Positive `consume_each` execution needs a specified transaction versus
script-group source, a resource Type Script identity, canonical cell-data
decoding, actual-count enforcement, and per-element predicate plus consumption
semantics. Positive `create_each` execution additionally needs a canonical
witness list codec, output ordering and one-to-one plan/output correspondence,
fresh Cell identity, and occupied-capacity rules. Until those contracts exist,
use explicit fixed-arity input/output bindings and ordinary lowered
`require`/`consume`/`create` operations.

## Authoring Guidance

Use dynamic vectors for data that is still a single cell field, such as signer
lists, proposal payload bytes, NFT attributes, or launch distributions.

Do not deploy ownership of multiple independent linear cells through a generic
vector, map, `BoundedCellSet`, or `BoundedList`. Use explicit fixed-arity action
parameters, named output bindings, and lowered `consume`/`destroy`/`create`
operations until the bounded runtime contract is implemented.

The missing verifier pieces are explicit cell consumption, typed collection
destructuring, and membership proofs tied to Molecule schema manifests. Until
those pieces exist, generic cell-backed collections stay outside the production
surface.
