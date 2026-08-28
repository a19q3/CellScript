# Four Months of CellScript: What Changed from 0.12 to 0.24

*A Q2 and Q3-to-date 2026 project report for the Nervos community*

When I posted CellScript 0.12 in April, I thought of it mainly as a compiler that had finally earned a clear release boundary. The bundled contracts compiled, the local CKB acceptance suite covered real transaction paths, and the compiler produced enough metadata for review. Handing the work to another team still required a dependable package, builder, and audit path.

By 0.24, the compiler is only one piece. `Cell.lock` records the exact dependencies. Generated builders carry transaction assumptions. The Registry stores source and artifact records. `cellc test` names the simulator or CKB-VM backend it actually ran. A separate checker reads the final ELF and its sidecars without loading the compiler that produced them.

The version numbers cover two uneven stretches of work. From April through June, the language and tooling began reporting CKB-specific facts directly. From July onward, package, builder, Registry, and audit paths had to preserve those facts as they moved outside the compiler.

## April to June: compiler and CKB boundaries

### 0.12: release scope

0.12 focused on documenting exactly what the release covered and making that result reproducible.

The bundled examples could compile to CKB-VM artifacts, produce metadata, and run through a local CKB acceptance path. Runtime failures had stable names and hints. The release record now carried CKB hash and CellDep requirements, the witness ABI, and transaction size and capacity evidence.

The claim covered the bundled suite. Arbitrary new contracts still required their own review and acceptance evidence. Because local acceptance logs can look like a production promise once copied out of the repository, every result carried its scope.

The original [0.12 community update](https://talk.nervos.org/t/cellscript-a-dsl-for-cell-based-contracts/10193/12) summarized the compiler and CKB gate work.

### 0.13–0.14: explicit Cell transitions

0.13 and 0.14 made the underlying CKB operations explicit in the language and its reports.

In 0.13, an action became easier to read as a proposed Cell transformation, with its inputs, outputs, and transition conditions visible in source. Lock-facing data sources became explicit. Standard lifecycle patterns moved out of compiler folklore and into reviewable library forms. A syntax-combination audit checked uncommon combinations across the front end, metadata, and generated artifact as well as the bundled happy paths.

0.14 brought CKB transaction sources and output data into the named contract boundary. Witness fields and Script args received distinct bindings; TYPE_ID, `since`, time, capacity, and child-verifier requirements became explicit records. Ordinary parameters kept their language-level role, with CKB data sources and authority represented separately.

An address supplied through a witness acquires authority only when a Lock Script verifies the relevant signature and message. A source-level capacity floor records a requirement; funding remains the builder's responsibility. TYPE_ID metadata records a construction plan, while commitment belongs to transaction and chain evidence. 0.14 put those distinctions in the reported contract.

The boundary took several passes to settle. The bundled `multisig.cell` example still carried discarded 64-byte signature-looking payloads. In 0.22 I removed them and renamed the remaining records as non-cryptographic approvals. The old example looked more convincing than it deserved.

### 0.15–0.16: ProofPlan and builder evidence

By May, CellScript could describe much more of a transaction, but each claim's discharge owner was still ambiguous: compiler, generated verifier, transaction builder, or chain.

A contract could state an invariant together with its trigger, scope, reads, and expected coverage. Cell identity and destruction stopped being implied by action names. ProofPlan gave reviewers one place to see the obligations behind an action or Lock Script.

The first version of that machinery recorded some aggregate claims for audit without generated runtime checks. Those entries were labelled “metadata-only” so reviewers could distinguish recorded intent from verifier coverage.

Hardening had found failure and status paths drifting into ordinary values, while lifecycle operations hidden inside branches received stronger descriptions than the analysis supported. I rejected those paths while the analysis caught up.

0.16 exposed those records to builders. A transaction builder could inspect the required Cells and CellDeps, witness and capacity duties, and signing assumptions before signing. Reviewers could compare proof and deployment facts between builds without diffing large metadata files by hand.

NovaSeal entered the repository in the same period as a serious proposal package with local evidence tooling. Its local devnet runs remained proposal evidence. Public Bitcoin SPV evidence, an independently reviewed BIP340 verifier, a live shared CellDep, and profile-specific attestations kept their own acceptance requirements. The separate [NovaSeal thread](https://talk.nervos.org/t/novaseal-a-bitcoin-authorised-cell-framework-for-ckb/10342) covers those protocol questions.

The 0.16 patch releases came directly from builder friction. 0.16.1 made the first Cell in token, launch, AMM, and NFT lifecycles explicit instead of asking an external builder to know a test harness convention.

0.16.2 fixed a concrete builder failure. A builder could take an active action artifact such as `token_mint_with_authority.elf` and use it as the passive Type Script identity of a new Token Cell. CKB then executed that action wrapper during Cell creation, the wrapper looked for action witness bytes, and the transaction failed with `entry-witness-abi-invalid`. The [external swap builder](https://github.com/WuodOdhis/cellscript-swap-builder/commit/479feb004338524d367b6656c6fb356ca7918f28) made the confusion impossible to dismiss as a documentation problem. 0.16.2 added compiler-owned passive resource identities and made the builder checks reject both scoped action artifacts and `always_success` fixtures in production shapes.

### 0.17–0.20: research and package integration

0.17, 0.18, and 0.19 formed one overlapping research and integration window. This report treats 0.20 as the public checkpoint.

The iCKB work compared selected CellScript behaviour with the original protocol under CKB-VM. Its production-equivalence status remained `NOT_PROVEN`; complete owner-authorisation fixtures, receipt decoding, DAO accounting, and production manifest closure remained open.

At the same time, the package and deployment model was changing underneath the compiler. By 0.20, a project included a source graph, manifest, lockfile, deployment record, and hashes connecting the build contract to deployed identity.

Multi-file packages and exact imports now worked as one source graph. Generated TypeScript builders could consume compiler metadata. Source packages could be installed and checked through the Registry path, deployment identity could be compared with live CKB RPC facts, and the adapter gave wallets and relayers a documented integration boundary. The browser Playground used the same package direction while compiling locally.

The [0.16–0.20 update](https://talk.nervos.org/t/cellscript-a-dsl-for-cell-based-contracts/10193/26) covers that package and build work in more detail.

## July and August: packages, Registry, and external evidence

Packages and builders moved evidence between people and tools, often long after the original compile. That hand-off needed a stable record in place of a folder of loosely related JSON files.

### 0.21–0.22: compile receipts and evidence tiers

Compile receipts put the source hash, metadata, ProofPlan, graph view, and artifact hashes in one record that the compiler or publisher could sign. ProtocolGraph showed state transitions beside their linked obligations without pretending to be a new consensus layer. Common xUDT conservation checks moved from descriptive metadata into executable coverage, and actions claiming a state transition had to use an edge declared by the corresponding flow.

An audit pipeline, builder, or Registry could now consume a stable record instead of reconstructing a build from loosely related files. The receipt signature authenticates the record. Proof ownership remains in the evidence tiers, while capacity, dry-run, and commitment require transaction and chain evidence.

0.21.1 was a documentation-only patch because the README had not fully caught up with the 0.21.0 release claim. The compiler and runtime were byte-identical to 0.21.0.

Every ProofPlan obligation received one of six tiers: checked by the compiler, checked by generated runtime code, waiting for a runtime helper, owed by the builder, metadata-only, or owed by chain evidence. Reviewers can now identify the owner of every outstanding obligation behind a green build.

The same rule applied to the language surface. Transaction reads became typed, read-only views; quantification and collection operations received explicit bounds; and capability rules became closed and versioned. Borrowed views had compile-time escape and lifecycle-authority checks. Forms without a finished runtime correspondence stayed fail-closed for production.

The release gate adopted the same ownership model. Separate reports covered compiler and builder results, runtime and transaction execution, artifact measurements, and chain evidence. The full gate pinned CKB source and binary provenance and covered the complete bundled action and Lock Script matrix.

### 0.23: Edition 2026 and the production Registry

0.23 changed less of the visible language than some earlier releases. Most of the work was operational: turning the Registry design into a running service.

Every package now had to say `edition = "2026"`. The edition covered source meaning, while target, assurance level, witness placement, and metadata schema kept their own versions. Compiler SemVer was limited to compiler release identity.

Parameterized CKB entries also gained one canonical home: `WitnessArgs.input_type` on the selected Script group. This removed an old raw witness shortcut and kept CellScript arguments away from the Lock Script's signature field. Existing projects migrated their builders and persisted records together.

The migration caught four CKB-VM crypto fixtures that still put raw `CSARGv1` bytes directly in the witness. They were rebuilt through `WitnessArgs.input_type`, and the raw form stayed as an explicit negative case. A permissive compatibility reader would have allowed the repository itself to keep using the shortcut.

The Registry could now accept a source package, queue isolated compiler verification, publish an immutable snapshot, and serve it to a fresh consumer. CellScript source packages remained dependency-resolving; executables and other artifact classes kept separate evidence and consumption paths.

User-facing paths shipped alongside the service. First publication gained a short browser authorisation flow while the private publishing key stayed in the local keychain. Pudge Testnet used a sandbox isolated from the production Registry. The Playground began preserving local work and the last successful output, with recovery after a compiler-worker failure.

The production work included database migration and immutable object storage. Verification jobs used bounded queues and leases. Backup and restore drills, environment separation, and worker-backed status reporting completed the service contract.

### 0.24: verified artifact bundles

After the 0.23 Registry went live, artifact explanation still depended mainly on the compiler that produced the artifact.

0.24 adds a smaller, compiler-independent checker. A CKB build is now a four-file bundle: the ELF, compile metadata, a canonical lowering record, and a canonical source map. The checker reads the bundle and recomputes a bounded set of structural facts without loading the compiler front end or code generator.

The checker validates ELF shape and control flow together with stack and ABI contracts. It also checks call and syscall use and binds hashes to source ranges. Business intent remains a separate review boundary.

0.24 also made `cellc test` name the backend it actually used: simulator, CKB-VM, or an explicit compile-only choice. Reports classify simulator results as development evidence and CKB-VM results as runtime evidence; deployment remains a separate state.

`Cell.lock` is now authoritative. Selection happens during an explicit lock or update operation, and ordinary builds consume the exact manifest, source, feature, test, and CKB environment graph. The lockfile migration removed mutable version selection from audited builds.

LS-IDL uses a similarly narrow boundary for Lock Script interfaces. The Registry stores the exact interface bytes and binds them to the executable and deployed Script identity. That establishes schema, suffix, and Script identity; implementation correctness and security review remain separate.

## Four months of change, side by side

| Area | 0.12 | 0.24 |
| --- | --- | --- |
| Unit of work | A compiler input and its bundled release context. | A manifest-bound package graph with exact runtime, test, feature, source, and environment identities. |
| Build output | CKB ELF or assembly plus a metadata sidecar. | A four-file CKB ELF bundle with metadata, verified lowering record, and source map. |
| Main trust anchor | The compiler, its metadata validator, and the local acceptance harness. | The compiler plus a separately packaged checker whose dependency graph excludes compiler front-end and code-generation components. |
| Testing | A strong bundled local CKB acceptance suite and compiler/policy tests. | Explicit simulator and CKB-VM package scenarios, kept separate from the stateful CKB release oracle and chain evidence. |
| Builder hand-off | ABI, witness, scheduler, constraint, and transaction-measurement reports. | Builder assumptions, transaction validation, generated builders, canonical witness placement, lock/deployment identity, and receipts. |
| Distribution | A narrow crates.io package and early Registry design discussion. | A live source/artifact Registry with immutable snapshots, evidence states, testnet isolation, and least-privilege artifact verification. |
| Interface discovery | Compiler ABI inspection and documentation. | Byte-exact LS-IDL publication and lookup by deployed Lock Script identity. |
| Compatibility | Compiler version carried most of the visible release identity. | Edition 2026, compiler SemVer, target, assurance, ABI, metadata schema, and package graph are separate versioned contracts. |
| Evidence language | Release gates separated bundled evidence from arbitrary-contract claims. | Binding, structure, lowering, VM execution, deployment, chain state, reproducibility, and semantic equivalence are explicitly separate states. |
| Ecosystem posture | Prove the bundled compiler surface works. | Let other tools consume the result while preserving the right to distrust and re-check it. |

## Next quarter: typed time and cross-Script composition

The last two releases spent most of their energy on package graphs, Registry workers, receipts, builders, and the independent checker. The next quarter shifts more effort back to the source language and application workflow.

The first source-level priority is [typed CKB time and `Since` values](https://github.com/CellScript-Labs/CellScript/issues/12). An epoch number, a block number, a timestamp, and an encoded `since` value are all `u64` today, although timelock, DAO, vesting, and atomic-swap contracts use them differently. The work covers the typed API, source migration, and updated example contracts, together with formatter, editor, metadata, builder, and CKB-VM coverage. The remaining [open issues](https://github.com/CellScript-Labs/CellScript/issues) stay outside that commitment unless a concrete fixture pulls them in.

The other main task is cross-application composition: several independent Lock and Type Scripts evaluate the same atomic CKB transaction, each under its own rules. CellScript can describe each artifact separately, but applications still have to merge their transaction requirements and prove the result against the selected deployments.

The reference fixture will use an order Type Script, a token Type Script, and an authorization Lock Script compiled as three separate artifacts. The first [ProtocolBundle](https://github.com/CellScript-Labs/CellScript/issues/9) will combine their builder contracts without recompiling them, reject conflicts before signing, and run every relevant Script Group against the same transaction bytes. Its negative cases cover witness-field conflicts, a wrong output index, a deployment from another network, and duplicate assignment of capacity or change.

The fixture will supply the requirements for [typed cross-Script roles](https://github.com/CellScript-Labs/CellScript/issues/10) and [exact interface-bound Script handles](https://github.com/CellScript-Labs/CellScript/issues/11). The first design will cover closed roles tied to known artifacts. Open roles depend on binding a Script selected during transaction construction to a checked interface without making an on-chain verifier trust a Registry lookup. `protocol` syntax can follow once that runtime contract is defined.

The settlement fixture also needs a generated TypeScript builder built around CCC. It will resolve named Cells and deployments, place canonical witnesses, and handle occupied capacity, fees, and change policy explicitly. It will then validate the finished transaction, dry-run every Script Group, and hand the unsigned transaction to a wallet. Key custody and signing policy stay with the wallet. Acceptance will run from a fresh external repository without setup code copied from CellScript's own fixtures.

The composition work is blocked on the [resolver bug](https://github.com/CellScript-Labs/CellScript/issues/17) that allows a transitive dependency to drift across CKB network identities. A stable [resolve graph and build plan](https://github.com/CellScript-Labs/CellScript/issues/19) will give builders and editors the same build identity. A [transactional upgrade plan](https://github.com/CellScript-Labs/CellScript/issues/20) will show package, interface, builder, and deployment changes before updating `Cell.lock`.

The language comparison will build the same timelock and settlement contracts in CellScript and Rust with `ckb-std`, adding a C/`ckb-c-stdlib` baseline where it helps. For the CKB implementations, the report will record source and test size, time to the first working CKB-VM test, pre-runtime errors, remaining builder code, artifact size, and cycles. Comparisons with Move and Sui Move, Cadence, Solidity and Vyper, Cairo, LIGO/Michelson, and Argent's app-linking work will cover ownership, transaction roles, interfaces, upgrades, and off-chain construction. VM cycle figures will stay within the comparable CKB implementations. All sources, fixtures, and rough edges will be public, including cases where CellScript loses.

Other candidates include [bounded Cell-group consumption](https://github.com/CellScript-Labs/CellScript/issues/7), [output correspondence](https://github.com/CellScript-Labs/CellScript/issues/8), and [typed committed substate](https://github.com/CellScript-Labs/CellScript/issues/13). They remain outside the quarter's commitment pending a concrete protocol, an accepted runtime contract, and independent review of the consensus-facing work.

The quarter has four testable deliverables: typed CKB time in real contracts; a three-Script settlement bundle; an external CCC builder that produces an unsigned, dry-run transaction from compiler metadata; and a reproducible language comparison with its complete source.

## Links

- [CellScript repository](https://github.com/CellScript-Labs/CellScript)
- [Original CellScript Nervos Talk thread](https://talk.nervos.org/t/cellscript-a-dsl-for-cell-based-contracts/10193)
- [0.16–0.20 public update](https://talk.nervos.org/t/cellscript-a-dsl-for-cell-based-contracts/10193/26)
- [CellScript 0.22 release notes](https://github.com/CellScript-Labs/CellScript/blob/v0.22.0/docs/releases/CELLSCRIPT_0_22_RELEASE_NOTES.md)
- [CellScript 0.23 release notes](https://github.com/CellScript-Labs/CellScript/blob/v0.23.0/docs/releases/CELLSCRIPT_0_23_RELEASE_NOTES.md)
- [CellScript 0.24 release notes](https://github.com/CellScript-Labs/CellScript/blob/v0.24.0/docs/releases/CELLSCRIPT_0_24_RELEASE_NOTES.md)
- [CellScript Playground](https://cellscript.dev/playground/)
