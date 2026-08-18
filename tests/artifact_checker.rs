use cellscript::{compile, CompileOptions, CompileResult};
use cellscript_artifact_checker::{
    canonical_bytes, canonical_hash, check_bundle, check_bundle_values, parse_elf, CheckerBudgets, CheckerRejectionCode, EdgeKind,
    SourceArtifactMap, TypedSemanticConstant, TypedSemanticOperationDetail, VerifiedLoweringRecord, LOWERING_RECORD_SCHEMA,
    SOURCE_MAP_SCHEMA,
};
use serde_json::Value;

const FIXTURE_SOURCE: &str = r#"
module artifact_checker_fixture

fn increment(value: u64) -> u64 {
    return value + 1
}

action main(value: u64) -> u64 {
    verification
        return increment(value)
}
"#;

#[derive(Clone)]
struct Fixture {
    artifact: Vec<u8>,
    metadata: Value,
    record: VerifiedLoweringRecord,
    source_map: SourceArtifactMap,
}

impl Fixture {
    fn new() -> Self {
        let result =
            compile(FIXTURE_SOURCE, CompileOptions { target: Some("riscv64-elf".to_string()), ..CompileOptions::default() }).unwrap();
        Self::from_result(result)
    }

    fn from_result(result: CompileResult) -> Self {
        let fixture = Self {
            artifact: result.artifact_bytes,
            metadata: serde_json::to_value(result.metadata).unwrap(),
            record: result.verified_lowering_record.unwrap(),
            source_map: result.source_artifact_map.unwrap(),
        };
        fixture.check().unwrap();
        fixture
    }

    fn from_source(source: &str) -> Self {
        let result = compile(source, CompileOptions { target: Some("riscv64-elf".to_string()), ..CompileOptions::default() }).unwrap();
        Self::from_result(result)
    }

    fn check(&self) -> Result<(), CheckerRejectionCode> {
        check_bundle_values(&self.artifact, &self.metadata, &self.record, &self.source_map, &CheckerBudgets::default())
            .map(|_| ())
            .map_err(|error| error.code)
    }

    fn rebind_sidecars(&mut self) {
        let record_hash = canonical_hash(LOWERING_RECORD_SCHEMA, &self.record).unwrap();
        self.source_map.lowering_record_hash = record_hash.clone();
        let source_map_hash = canonical_hash(SOURCE_MAP_SCHEMA, &self.source_map).unwrap();
        self.metadata["verified_artifact"]["lowering_record_hash"] = Value::String(record_hash);
        self.metadata["verified_artifact"]["source_map_hash"] = Value::String(source_map_hash);
    }

    fn rebind_typed_semantics(&mut self) {
        self.record.typed_semantics_hash =
            canonical_hash(cellscript_artifact_checker::TYPED_SEMANTICS_SCHEMA, &self.record.typed_semantics).unwrap();
        self.metadata["typed_semantics"] = serde_json::to_value(&self.record.typed_semantics).unwrap();
        self.metadata["typed_semantics_hash"] = Value::String(self.record.typed_semantics_hash.clone());
        self.rebind_sidecars();
    }

    fn bind_artifact_identity(&mut self) {
        let artifact_hash = cellscript_artifact_checker::hex_encode(&cellscript_artifact_checker::ckb_blake2b256(&self.artifact));
        self.record.artifact_hash.clone_from(&artifact_hash);
        self.record.artifact_size_bytes = self.artifact.len() as u64;
        self.source_map.artifact_hash.clone_from(&artifact_hash);
        self.metadata["artifact_hash"] = Value::String(artifact_hash);
        self.metadata["artifact_size_bytes"] = Value::from(self.artifact.len() as u64);
        self.rebind_sidecars();
    }
}

const REFERENCE_ENTRY_SOURCE: &str = r#"
module artifact_checker_reference_fixture

resource Token has consume {
    amount: u64,
}

fn inspect(token: &Token) -> u64 {
    return token.amount
}

action main() -> u64 {
    verification
        return 0
}
"#;

fn assert_code(fixture: &Fixture, expected: CheckerRejectionCode) {
    match fixture.check() {
        Ok(()) => panic!("mutation unexpectedly passed; expected {}", expected.as_str()),
        Err(actual) => assert_eq!(actual, expected),
    }
}

#[test]
fn verified_artifact_sidecars_are_deterministic_and_canonical() {
    let first = Fixture::new();
    let second = Fixture::new();
    assert_eq!(first.artifact, second.artifact);
    assert_eq!(canonical_bytes(&first.record).unwrap(), canonical_bytes(&second.record).unwrap());
    assert_eq!(canonical_bytes(&first.source_map).unwrap(), canonical_bytes(&second.source_map).unwrap());
    assert!(first.source_map.intervals.iter().all(|interval| interval.source_path == "<memory>"));
}

#[test]
fn stable_rejection_codes_cover_json_budget_graph_abi_proof_and_binding_mutations() {
    let valid = Fixture::new();
    let budgets = CheckerBudgets::default();
    let metadata_bytes = serde_json::to_vec(&valid.metadata).unwrap();
    let record_bytes = canonical_bytes(&valid.record).unwrap();
    let source_map_bytes = canonical_bytes(&valid.source_map).unwrap();

    let mut tiny = budgets.clone();
    tiny.artifact_bytes = 1;
    assert_eq!(
        check_bundle(&valid.artifact, &metadata_bytes, &record_bytes, &source_map_bytes, &tiny).unwrap_err().code,
        CheckerRejectionCode::V2400BudgetExceeded,
    );
    assert_eq!(
        check_bundle(&valid.artifact, &metadata_bytes, b"{", &source_map_bytes, &budgets).unwrap_err().code,
        CheckerRejectionCode::V2401MalformedJson,
    );
    assert_eq!(
        check_bundle(
            &valid.artifact,
            &metadata_bytes,
            &serde_json::to_vec_pretty(&valid.record).unwrap(),
            &source_map_bytes,
            &budgets,
        )
        .unwrap_err()
        .code,
        CheckerRejectionCode::V2402NonCanonicalJson,
    );

    let mut changed = valid.clone();
    changed.record.schema = "future-schema".to_string();
    assert_code(&changed, CheckerRejectionCode::V2403UnsupportedSchema);

    let mut changed = valid.clone();
    changed.record.entries[0].id = "zz-noncanonical".to_string();
    changed.rebind_sidecars();
    assert_code(&changed, CheckerRejectionCode::V2404CanonicalOrder);

    let mut changed = valid.clone();
    changed.record.entries[0].entry_block = "missing:block".to_string();
    changed.rebind_sidecars();
    assert_code(&changed, CheckerRejectionCode::V2405ReferentialIntegrity);

    let mut changed = valid.clone();
    let index = changed
        .record
        .blocks
        .iter()
        .position(|block| changed.record.edges.iter().any(|edge| edge.from == block.id && edge.kind != EdgeKind::Call))
        .expect("fixture must contain a non-return CFG edge");
    changed.record.blocks[index].terminator = cellscript_artifact_checker::MachineTerminator::Return;
    changed.rebind_sidecars();
    assert_code(&changed, CheckerRejectionCode::V2406CfgInvalid);

    let mut changed = valid.clone();
    changed.record.runtime_error_exits.push(cellscript_artifact_checker::RuntimeErrorExit {
        block_id: changed.record.blocks[0].id.clone(),
        address: changed.record.blocks[0].range.end,
        code: 5,
        name: "assertion-failed".to_string(),
    });
    changed.record.canonicalize();
    changed.rebind_sidecars();
    assert_code(&changed, CheckerRejectionCode::V2406CfgInvalid);

    let mut changed = valid.clone();
    changed.record.blocks[0].reachable = !changed.record.blocks[0].reachable;
    changed.rebind_sidecars();
    assert_code(&changed, CheckerRejectionCode::V2406CfgInvalid);

    let mut changed = valid.clone();
    let param = changed.record.entries.iter_mut().find_map(|entry| entry.params.first_mut()).unwrap();
    param.alignment_bytes = 3;
    changed.rebind_sidecars();
    assert_code(&changed, CheckerRejectionCode::V2407AbiOrStackInvalid);

    let mut changed = valid.clone();
    let framed_entry =
        changed.record.entries.iter().position(|entry| entry.frame_size_bytes > 0).expect("fixture must contain a stack-framed entry");
    let owner = changed.record.entries[framed_entry].id.clone();
    changed.record.entries[framed_entry].frame_size_bytes = 0;
    changed.record.entries[framed_entry].outgoing_argument_bytes = 0;
    for block in changed.record.blocks.iter_mut().filter(|block| block.owner_entry == owner) {
        block.frame_size_bytes = 0;
        block.outgoing_argument_bytes = 0;
        block.stack_slots.clear();
    }
    changed.rebind_sidecars();
    assert_code(&changed, CheckerRejectionCode::V2407AbiOrStackInvalid);

    let mut changed = valid.clone();
    changed.record.entries[0].proof_ids.push("zz-missing-proof".to_string());
    changed.record.entries[0].proof_ids.sort();
    changed.rebind_sidecars();
    assert_code(&changed, CheckerRejectionCode::V2408ProofCoverageInvalid);

    let mut changed = valid.clone();
    changed.artifact[0] ^= 1;
    assert_code(&changed, CheckerRejectionCode::V2409ArtifactIdentityMismatch);

    let mut changed = valid.clone();
    changed.metadata["module"] = Value::String("tampered".to_string());
    assert_code(&changed, CheckerRejectionCode::V2410MetadataBindingMismatch);

    let mut changed = valid.clone();
    changed.record.typed_semantics.entries[0].effect = "tampered".to_string();
    changed.rebind_sidecars();
    assert_code(&changed, CheckerRejectionCode::V2419TypedSemanticsInvalid);

    let mut changed = valid.clone();
    let typed_param = changed
        .record
        .typed_semantics
        .entries
        .iter_mut()
        .find_map(|entry| entry.params.first_mut())
        .expect("fixture must contain a typed parameter");
    typed_param.ty = "u128".to_string();
    let binding_id = typed_param.binding_id;
    let typed_local = changed
        .record
        .typed_semantics
        .entries
        .iter_mut()
        .flat_map(|entry| entry.locals.iter_mut())
        .find(|local| local.id == binding_id)
        .expect("typed parameter must bind a local");
    typed_local.ty = "u128".to_string();
    changed.record.typed_semantics_hash =
        canonical_hash(cellscript_artifact_checker::TYPED_SEMANTICS_SCHEMA, &changed.record.typed_semantics).unwrap();
    changed.metadata["typed_semantics"] = serde_json::to_value(&changed.record.typed_semantics).unwrap();
    changed.metadata["typed_semantics_hash"] = Value::String(changed.record.typed_semantics_hash.clone());
    changed.rebind_sidecars();
    assert_code(&changed, CheckerRejectionCode::V2420TypedMachineBindingInvalid);

    let mut changed = valid.clone();
    if let Some(interval) = changed.source_map.intervals.first_mut() {
        interval.source_path = "../escape.cell".to_string();
    } else {
        changed.source_map.schema = "bad-map".to_string();
    }
    changed.rebind_sidecars();
    assert_code(&changed, CheckerRejectionCode::V2416SourceMapInvalid);

    let mut changed = valid.clone();
    if let Some(site) = changed.record.syscall_sites.first_mut() {
        site.contract.clear();
    } else {
        changed.record.syscall_sites.push(cellscript_artifact_checker::SyscallSite {
            block_id: changed.record.blocks[0].id.clone(),
            address: changed.record.blocks[0].range.start,
            syscall_number: None,
            contract: "declared-but-not-present".to_string(),
            source_domain: "test".to_string(),
            index_domain: "test".to_string(),
            return_code_checked: true,
            buffer_limit_bytes: 1,
        });
    }
    changed.rebind_sidecars();
    assert_code(&changed, CheckerRejectionCode::V2417SyscallContractInvalid);

    let mut changed = valid.clone();
    let distinct = changed
        .record
        .entries
        .iter()
        .enumerate()
        .find_map(|(left, a)| changed.record.entries.iter().enumerate().find(|(_, b)| b.id != a.id).map(|(right, _)| (left, right)))
        .unwrap();
    let left = changed.record.entries[distinct.0].entry_block.clone();
    let right = changed.record.entries[distinct.1].entry_block.clone();
    changed.record.edges.push(cellscript_artifact_checker::LoweringEdge {
        from: left.clone(),
        to: right.clone(),
        kind: EdgeKind::Call,
    });
    changed.record.edges.push(cellscript_artifact_checker::LoweringEdge { from: right, to: left, kind: EdgeKind::Call });
    changed.record.canonicalize();
    changed.rebind_sidecars();
    assert_code(&changed, CheckerRejectionCode::V2418RecursionPolicyInvalid);
}

#[test]
fn stable_rejection_codes_cover_elf_sections_instructions_flow_and_digests() {
    let valid = Fixture::new();

    let mut changed = valid.clone();
    changed.artifact[0] = 0;
    changed.bind_artifact_identity();
    assert_code(&changed, CheckerRejectionCode::V2411ElfFormatInvalid);

    let mut changed = valid.clone();
    let section_table = u64::from_le_bytes(changed.artifact[40..48].try_into().unwrap()) as usize;
    let rodata_type = section_table + 2 * 64 + 4;
    changed.artifact[rodata_type..rodata_type + 4].copy_from_slice(&6_u32.to_le_bytes());
    changed.bind_artifact_identity();
    assert_code(&changed, CheckerRejectionCode::V2412ElfSectionInvalid);

    let elf = parse_elf(&valid.artifact, CheckerBudgets::default().instructions).unwrap();
    let text_offset = elf.text.offset as usize;

    let mut changed = valid.clone();
    changed.artifact[text_offset..text_offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    changed.bind_artifact_identity();
    assert_code(&changed, CheckerRejectionCode::V2413InstructionInvalid);

    let mut changed = valid.clone();
    changed.artifact[text_offset..text_offset + 4].copy_from_slice(&encode_jal(1_048_574).to_le_bytes());
    changed.bind_artifact_identity();
    assert_code(&changed, CheckerRejectionCode::V2414ControlFlowInvalid);

    let mut changed = valid.clone();
    let candidate = elf
        .instructions
        .windows(2)
        .find(|pair| {
            let word = pair[0].word;
            let rd = (word >> 7) & 0x1f;
            let next = pair[1].word;
            let next_uses_rd_for_sp =
                next & 0x7f == 0x33 && (next >> 7) & 0x1f == 2 && (next >> 15) & 0x1f == 2 && (next >> 20) & 0x1f == rd;
            valid.record.text_range.contains(pair[0].address)
                && word & 0x7f == 0x13
                && rd != 2
                && !next_uses_rd_for_sp
                && valid
                    .record
                    .blocks
                    .iter()
                    .any(|block| block.range.contains(pair[0].address) && pair[0].address + 4 < block.range.end)
        })
        .map(|pair| pair[0])
        .expect("fixture must contain a non-terminating add-immediate instruction");
    let block_offset = elf.text.offset as usize + (candidate.address - elf.text.address) as usize;
    changed.artifact[block_offset..block_offset + 4].copy_from_slice(&(candidate.word ^ (1 << 20)).to_le_bytes());
    changed.bind_artifact_identity();
    assert_code(&changed, CheckerRejectionCode::V2415BlockDigestMismatch);
}

#[test]
fn typed_semantics_rejects_operator_and_constant_mutations_after_rebinding() {
    let valid = Fixture::new();

    let mut changed = valid.clone();
    let binary = changed
        .record
        .typed_semantics
        .entries
        .iter_mut()
        .flat_map(|entry| &mut entry.blocks)
        .flat_map(|block| &mut block.operations)
        .find(|operation| operation.opcode == "binary")
        .expect("fixture must contain a binary operation");
    binary.detail = TypedSemanticOperationDetail::BinaryOperator { operator: "and".to_string() };
    changed.rebind_typed_semantics();
    assert_code(&changed, CheckerRejectionCode::V2419TypedSemanticsInvalid);

    let mut changed = valid.clone();
    let constant = changed
        .record
        .typed_semantics
        .entries
        .iter_mut()
        .flat_map(|entry| &mut entry.blocks)
        .flat_map(|block| &mut block.operations)
        .flat_map(|operation| &mut operation.operands)
        .find_map(|operand| operand.constant.as_mut())
        .expect("fixture must contain a constant operand");
    *constant = TypedSemanticConstant::U64("01".to_string());
    changed.rebind_typed_semantics();
    assert_code(&changed, CheckerRejectionCode::V2419TypedSemanticsInvalid);

    let mut changed = valid.clone();
    let constant = changed
        .record
        .typed_semantics
        .entries
        .iter_mut()
        .flat_map(|entry| &mut entry.blocks)
        .flat_map(|block| &mut block.operations)
        .flat_map(|operation| &mut operation.operands)
        .find_map(|operand| match &mut operand.constant {
            Some(TypedSemanticConstant::U64(value)) => Some(value),
            _ => None,
        })
        .expect("fixture must contain a u64 constant operand");
    *constant = "2".to_string();
    changed.rebind_typed_semantics();
    assert_code(&changed, CheckerRejectionCode::V2420TypedMachineBindingInvalid);
}

#[test]
fn typed_semantics_accepts_declared_vec_constructors_and_unsigned_widening() {
    let widened = Fixture::from_source(
        r#"
module checker::widening

action multiply(amount: u64, basis_points: u16) -> u64 {
    verification
        return amount * basis_points
}
"#,
    );
    assert!(widened.check().is_ok());

    let empty_array = Fixture::from_source(
        r#"
module checker::empty_array

action empty() -> [u8; 0] {
    verification
        return []
}
"#,
    );
    assert!(empty_array.check().is_ok());

    let script_tuple = Fixture::from_source(
        r#"
module checker::script_tuple

action script_value() -> u64 {
    verification
        let args = script::args_empty()
        let value = script::new(Hash::zero(), 0, args)
        return 0
}
"#,
    );
    assert!(script_tuple.check().is_ok());

    let collection = Fixture::from_source(include_str!("../examples/language/order_book.cell"));
    assert!(collection.check().is_ok());

    let mut changed = collection;
    let declared_type = changed
        .record
        .typed_semantics
        .entries
        .iter_mut()
        .flat_map(|entry| &mut entry.blocks)
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.detail {
            TypedSemanticOperationDetail::Collection { declared_type } => Some(declared_type),
            _ => None,
        })
        .expect("fixture must contain a collection constructor");
    *declared_type = "Map".to_string();
    changed.rebind_typed_semantics();
    assert_code(&changed, CheckerRejectionCode::V2419TypedSemanticsInvalid);
}

#[test]
fn typed_semantics_requires_reference_coercion_for_reference_calls() {
    let valid = Fixture::from_source(
        r#"
module checker::reference_call

struct Wallet {
    amount: u64,
}

fn inspect(wallet: &Wallet) -> u64 {
    return wallet.amount
}

action main(wallet: Wallet) -> u64 {
    verification
        return inspect(&wallet)
}
"#,
    );
    assert!(valid.check().is_ok());

    let mut changed = valid;
    let operator = changed
        .record
        .typed_semantics
        .entries
        .iter_mut()
        .flat_map(|entry| &mut entry.blocks)
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.detail {
            TypedSemanticOperationDetail::UnaryOperator { operator } if operator == "ref" => Some(operator),
            _ => None,
        })
        .expect("fixture must contain a reference coercion");
    *operator = "deref".to_string();
    changed.rebind_typed_semantics();
    assert_code(&changed, CheckerRejectionCode::V2419TypedSemanticsInvalid);
}

#[test]
fn typed_semantics_requires_exact_guard_for_unsigned_narrowing() {
    let valid = Fixture::from_source(
        r#"
module checker::narrowing

action narrow(value: u64) -> u8 {
    verification
        return value as u8
}
"#,
    );
    assert!(valid.check().is_ok());

    let mut changed = valid;
    let bound = changed
        .record
        .typed_semantics
        .entries
        .iter_mut()
        .flat_map(|entry| &mut entry.blocks)
        .flat_map(|block| &mut block.operations)
        .flat_map(|operation| &mut operation.operands)
        .find_map(|operand| match &mut operand.constant {
            Some(TypedSemanticConstant::U64(value)) if value == "255" => Some(value),
            _ => None,
        })
        .expect("fixture must contain the u8 narrowing bound");
    *bound = "256".to_string();
    changed.rebind_typed_semantics();
    assert_code(&changed, CheckerRejectionCode::V2419TypedSemanticsInvalid);
}

#[test]
fn typed_semantics_rejects_field_enum_layout_and_instantiation_mutations() {
    let valid = Fixture::from_source(include_str!("syntax_combo/seeds/generic-value.cell"));

    let mut changed = valid.clone();
    let field = changed
        .record
        .typed_semantics
        .entries
        .iter_mut()
        .flat_map(|entry| &mut entry.blocks)
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.detail {
            TypedSemanticOperationDetail::Field { name } => Some(name),
            _ => None,
        })
        .expect("fixture must contain a field access");
    *field = "missing".to_string();
    changed.rebind_typed_semantics();
    assert_code(&changed, CheckerRejectionCode::V2419TypedSemanticsInvalid);

    let mut changed = valid.clone();
    let variant = changed
        .record
        .typed_semantics
        .entries
        .iter_mut()
        .flat_map(|entry| &mut entry.blocks)
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.detail {
            TypedSemanticOperationDetail::EnumConstruct { variant, .. } => Some(variant),
            _ => None,
        })
        .expect("fixture must contain an enum constructor");
    *variant = "Missing".to_string();
    changed.rebind_typed_semantics();
    assert_code(&changed, CheckerRejectionCode::V2419TypedSemanticsInvalid);

    let mut changed = valid.clone();
    let field_index = changed
        .record
        .typed_semantics
        .entries
        .iter_mut()
        .flat_map(|entry| &mut entry.blocks)
        .flat_map(|block| &mut block.operations)
        .find_map(|operation| match &mut operation.detail {
            TypedSemanticOperationDetail::EnumPayload { field_index, .. } => Some(field_index),
            _ => None,
        })
        .expect("fixture must contain an enum payload read");
    *field_index = u32::MAX;
    changed.rebind_typed_semantics();
    assert_code(&changed, CheckerRejectionCode::V2419TypedSemanticsInvalid);

    let mut changed = valid.clone();
    changed.record.typed_semantics.types[0].layout_hash = "00".repeat(32);
    changed.rebind_typed_semantics();
    assert_code(&changed, CheckerRejectionCode::V2419TypedSemanticsInvalid);

    let mut changed = valid.clone();
    changed.record.typed_semantics.instantiations[0].identity.push_str("::tampered");
    changed.rebind_typed_semantics();
    assert_code(&changed, CheckerRejectionCode::V2419TypedSemanticsInvalid);
}

#[test]
fn typed_semantics_rejects_borrow_ownership_cfg_and_reference_mutations() {
    let valid = Fixture::from_source(include_str!("syntax_combo/seeds/explicit-borrow.cell"));

    let mut changed = valid.clone();
    changed.record.typed_semantics.entries[0].borrows[0].start_operation = u32::MAX;
    changed.rebind_typed_semantics();
    assert_code(&changed, CheckerRejectionCode::V2419TypedSemanticsInvalid);

    let mut changed = valid.clone();
    changed.record.typed_semantics.entries[0].ownership[0].final_state = "available".to_string();
    changed.rebind_typed_semantics();
    assert_code(&changed, CheckerRejectionCode::V2419TypedSemanticsInvalid);

    let mut changed = valid.clone();
    let block = changed
        .record
        .typed_semantics
        .entries
        .iter_mut()
        .flat_map(|entry| &mut entry.blocks)
        .find(|block| !block.successors.is_empty())
        .expect("fixture must contain a CFG edge");
    block.successors[0] = u32::MAX;
    changed.rebind_typed_semantics();
    assert_code(&changed, CheckerRejectionCode::V2419TypedSemanticsInvalid);

    let mut changed = Fixture::from_source(REFERENCE_ENTRY_SOURCE);
    let entry = changed
        .record
        .typed_semantics
        .entries
        .iter_mut()
        .find(|entry| entry.name == "inspect")
        .expect("fixture must contain the reference helper");
    let binding_id = entry.params[0].binding_id;
    entry.params[0].ty = "Token".to_string();
    entry.locals.iter_mut().find(|local| local.id == binding_id).unwrap().ty = "Token".to_string();
    for operand in entry.blocks.iter_mut().flat_map(|block| &mut block.operations).flat_map(|operation| &mut operation.operands) {
        if operand.local == Some(binding_id) {
            operand.ty = "Token".to_string();
        }
    }
    changed.rebind_typed_semantics();
    assert_code(&changed, CheckerRejectionCode::V2420TypedMachineBindingInvalid);
}

fn encode_jal(offset: i32) -> u32 {
    let immediate = offset as u32;
    (((immediate >> 20) & 1) << 31)
        | (((immediate >> 1) & 0x03ff) << 21)
        | (((immediate >> 11) & 1) << 20)
        | (((immediate >> 12) & 0x00ff) << 12)
        | 0x6f
}
