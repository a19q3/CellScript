use cellscript::{compile, CompileOptions, CompileResult};
use cellscript_artifact_checker::{
    canonical_bytes, canonical_hash, check_bundle, check_bundle_values, parse_elf, CheckerBudgets, CheckerRejectionCode, EdgeKind,
    SourceArtifactMap, VerifiedLoweringRecord, LOWERING_RECORD_SCHEMA, SOURCE_MAP_SCHEMA,
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

fn encode_jal(offset: i32) -> u32 {
    let immediate = offset as u32;
    (((immediate >> 20) & 1) << 31)
        | (((immediate >> 1) & 0x03ff) << 21)
        | (((immediate >> 11) & 1) << 20)
        | (((immediate >> 12) & 0x00ff) << 12)
        | 0x6f
}
