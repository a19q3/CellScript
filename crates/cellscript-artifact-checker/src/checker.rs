use crate::elf::{parse_elf, DecodedControlFlowKind, ElfErrorKind, ElfParseError, ElfSummary, ParsedElf};
use crate::schema::*;
use crate::{ckb_blake2b256, hex_encode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckerRejectionCode {
    V2400BudgetExceeded,
    V2401MalformedJson,
    V2402NonCanonicalJson,
    V2403UnsupportedSchema,
    V2404CanonicalOrder,
    V2405ReferentialIntegrity,
    V2406CfgInvalid,
    V2407AbiOrStackInvalid,
    V2408ProofCoverageInvalid,
    V2409ArtifactIdentityMismatch,
    V2410MetadataBindingMismatch,
    V2411ElfFormatInvalid,
    V2412ElfSectionInvalid,
    V2413InstructionInvalid,
    V2414ControlFlowInvalid,
    V2415BlockDigestMismatch,
    V2416SourceMapInvalid,
    V2417SyscallContractInvalid,
    V2418RecursionPolicyInvalid,
}

impl CheckerRejectionCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::V2400BudgetExceeded => "V2400",
            Self::V2401MalformedJson => "V2401",
            Self::V2402NonCanonicalJson => "V2402",
            Self::V2403UnsupportedSchema => "V2403",
            Self::V2404CanonicalOrder => "V2404",
            Self::V2405ReferentialIntegrity => "V2405",
            Self::V2406CfgInvalid => "V2406",
            Self::V2407AbiOrStackInvalid => "V2407",
            Self::V2408ProofCoverageInvalid => "V2408",
            Self::V2409ArtifactIdentityMismatch => "V2409",
            Self::V2410MetadataBindingMismatch => "V2410",
            Self::V2411ElfFormatInvalid => "V2411",
            Self::V2412ElfSectionInvalid => "V2412",
            Self::V2413InstructionInvalid => "V2413",
            Self::V2414ControlFlowInvalid => "V2414",
            Self::V2415BlockDigestMismatch => "V2415",
            Self::V2416SourceMapInvalid => "V2416",
            Self::V2417SyscallContractInvalid => "V2417",
            Self::V2418RecursionPolicyInvalid => "V2418",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckerError {
    pub code: CheckerRejectionCode,
    pub message: String,
}

impl CheckerError {
    fn new(code: CheckerRejectionCode, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }

    fn bounded(mut self, max_bytes: u32) -> Self {
        let max_bytes = usize::try_from(max_bytes).unwrap_or(usize::MAX);
        if self.message.len() > max_bytes {
            let mut end = max_bytes.min(self.message.len());
            while end > 0 && !self.message.is_char_boundary(end) {
                end -= 1;
            }
            self.message.truncate(end);
        }
        self
    }
}

impl std::fmt::Display for CheckerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for CheckerError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceState {
    Verified,
    NotProvided,
    NotExecuted,
    NotClaimed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckerReport {
    pub schema: String,
    pub checker_name: String,
    pub checker_version: String,
    pub checker_policy_schema: String,
    pub artifact_hash: String,
    pub lowering_record_hash: String,
    pub source_map_hash: String,
    pub binding_verification: EvidenceState,
    pub structural_verification: EvidenceState,
    pub lowering_record_verification: EvidenceState,
    pub ckb_vm_evidence: EvidenceState,
    pub chain_evidence: EvidenceState,
    pub semantic_equivalence_claimed: bool,
    pub elf: ElfSummary,
}

pub fn canonical_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, CheckerError> {
    serde_json::to_vec(value).map_err(|error| {
        CheckerError::new(CheckerRejectionCode::V2401MalformedJson, format!("failed to serialize canonical checker value: {error}"))
    })
}

pub fn canonical_hash<T: Serialize>(domain: &str, value: &T) -> Result<String, CheckerError> {
    let bytes = canonical_bytes(value)?;
    let mut material = Vec::with_capacity(domain.len() + 1 + bytes.len());
    material.extend_from_slice(domain.as_bytes());
    material.push(0);
    material.extend_from_slice(&bytes);
    Ok(hex_encode(&ckb_blake2b256(&material)))
}

pub fn parse_lowering_record(bytes: &[u8], budgets: &CheckerBudgets) -> Result<VerifiedLoweringRecord, CheckerError> {
    ensure_byte_budget("lowering record", bytes.len(), budgets.record_bytes)?;
    let record: VerifiedLoweringRecord = serde_json::from_slice(bytes).map_err(|error| {
        CheckerError::new(CheckerRejectionCode::V2401MalformedJson, format!("failed to parse lowering record: {error}"))
    })?;
    ensure_canonical("lowering record", bytes, &record)?;
    Ok(record)
}

pub fn parse_source_map(bytes: &[u8], budgets: &CheckerBudgets) -> Result<SourceArtifactMap, CheckerError> {
    ensure_byte_budget("source map", bytes.len(), budgets.source_map_bytes)?;
    let source_map: SourceArtifactMap = serde_json::from_slice(bytes).map_err(|error| {
        CheckerError::new(CheckerRejectionCode::V2401MalformedJson, format!("failed to parse source map: {error}"))
    })?;
    ensure_canonical("source map", bytes, &source_map)?;
    Ok(source_map)
}

pub fn check_bundle(
    artifact: &[u8],
    metadata_bytes: &[u8],
    lowering_record_bytes: &[u8],
    source_map_bytes: &[u8],
    budgets: &CheckerBudgets,
) -> Result<CheckerReport, CheckerError> {
    let result = (|| {
        if budgets.schema != CHECKER_POLICY_SCHEMA {
            return Err(CheckerError::new(
                CheckerRejectionCode::V2403UnsupportedSchema,
                format!("unsupported checker policy schema '{}'", budgets.schema),
            ));
        }
        ensure_byte_budget("artifact", artifact.len(), budgets.artifact_bytes)?;
        let metadata: Value = serde_json::from_slice(metadata_bytes).map_err(|error| {
            CheckerError::new(CheckerRejectionCode::V2401MalformedJson, format!("failed to parse compile metadata: {error}"))
        })?;
        let record = parse_lowering_record(lowering_record_bytes, budgets)?;
        let source_map = parse_source_map(source_map_bytes, budgets)?;
        check_bundle_values(artifact, &metadata, &record, &source_map, budgets)
    })();
    result.map_err(|error| error.bounded(budgets.diagnostic_bytes))
}

pub fn check_bundle_values(
    artifact: &[u8],
    metadata: &Value,
    record: &VerifiedLoweringRecord,
    source_map: &SourceArtifactMap,
    budgets: &CheckerBudgets,
) -> Result<CheckerReport, CheckerError> {
    validate_record_schema(record)?;
    validate_declared_limits(&record.limits, budgets)?;
    validate_counts(record, source_map, budgets)?;
    validate_metadata_binding(artifact, metadata, record, source_map)?;
    validate_record_graph(record, budgets)?;

    let elf = parse_elf(artifact, budgets.instructions).map_err(map_elf_error)?;
    validate_elf_binding(artifact, record, &elf)?;
    validate_block_digests(artifact, record, &elf)?;
    validate_control_flow(record, &elf)?;
    validate_machine_terminators(record, &elf)?;
    validate_stack_discipline(record, &elf)?;
    validate_syscalls(record, &elf)?;
    validate_source_map(source_map, record, artifact, &elf)?;

    Ok(CheckerReport {
        schema: CHECKER_REPORT_SCHEMA.to_string(),
        checker_name: "cellscript-artifact-checker".to_string(),
        checker_version: CHECKER_VERSION.to_string(),
        checker_policy_schema: budgets.schema.clone(),
        artifact_hash: record.artifact_hash.clone(),
        lowering_record_hash: canonical_hash(LOWERING_RECORD_SCHEMA, record)?,
        source_map_hash: canonical_hash(SOURCE_MAP_SCHEMA, source_map)?,
        binding_verification: EvidenceState::Verified,
        structural_verification: EvidenceState::Verified,
        lowering_record_verification: EvidenceState::Verified,
        ckb_vm_evidence: EvidenceState::NotExecuted,
        chain_evidence: EvidenceState::NotProvided,
        semantic_equivalence_claimed: false,
        elf: elf.summary(),
    })
}

fn validate_record_schema(record: &VerifiedLoweringRecord) -> Result<(), CheckerError> {
    if record.schema != LOWERING_RECORD_SCHEMA || record.version != LOWERING_RECORD_VERSION {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2403UnsupportedSchema,
            format!("unsupported lowering record '{}'/{}", record.schema, record.version),
        ));
    }
    if record.claim.lowering_record != "binding-verified"
        || record.claim.machine_code != "structurally-verified"
        || record.claim.semantic_equivalence
    {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2403UnsupportedSchema,
            "lowering record overclaims or mislabels the v1 verification boundary",
        ));
    }
    if record.artifact_format != "RISC-V ELF" || record.target_profile != "ckb" {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2403UnsupportedSchema,
            "v1 checker accepts only the CKB RISC-V ELF profile",
        ));
    }
    if record.compatibility_profile.target_profile != record.target_profile
        || record.compatibility_profile.edition != record.edition
        || record.compatibility_profile.raw_entry_witness_payload_compatible
    {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2410MetadataBindingMismatch,
            "record compatibility profile disagrees with edition/target or accepts raw entry witnesses",
        ));
    }
    let profile_hash = canonical_hash("cellscript-compatibility-profile-identity-v1", &record.compatibility_profile)?;
    if profile_hash != record.compatibility_profile_hash {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2410MetadataBindingMismatch,
            "record compatibility profile hash does not match its canonical identity",
        ));
    }
    Ok(())
}

fn validate_declared_limits(declared: &DeclaredLimits, budgets: &CheckerBudgets) -> Result<(), CheckerError> {
    let checks = [
        ("artifact_bytes", declared.artifact_bytes, budgets.artifact_bytes),
        ("record_bytes", declared.record_bytes, budgets.record_bytes),
        ("source_map_bytes", declared.source_map_bytes, budgets.source_map_bytes),
        ("entries", u64::from(declared.entries), u64::from(budgets.entries)),
        ("blocks", u64::from(declared.blocks), u64::from(budgets.blocks)),
        ("edges", u64::from(declared.edges), u64::from(budgets.edges)),
        ("instructions", declared.instructions, budgets.instructions),
        ("call_depth", u64::from(declared.call_depth), u64::from(budgets.call_depth)),
        ("stack_frame_bytes", u64::from(declared.stack_frame_bytes), u64::from(budgets.stack_frame_bytes)),
        ("proof_records", u64::from(declared.proof_records), u64::from(budgets.proof_records)),
        ("source_map_intervals", u64::from(declared.source_map_intervals), u64::from(budgets.source_map_intervals)),
        ("diagnostic_bytes", u64::from(declared.diagnostic_bytes), u64::from(budgets.diagnostic_bytes)),
    ];
    for (name, value, limit) in checks {
        if value > limit {
            return Err(CheckerError::new(
                CheckerRejectionCode::V2400BudgetExceeded,
                format!("record-declared {name} limit {value} exceeds checker policy {limit}"),
            ));
        }
    }
    Ok(())
}

fn validate_counts(
    record: &VerifiedLoweringRecord,
    source_map: &SourceArtifactMap,
    budgets: &CheckerBudgets,
) -> Result<(), CheckerError> {
    ensure_count("entries", record.entries.len(), budgets.entries)?;
    ensure_count("blocks", record.blocks.len(), budgets.blocks)?;
    ensure_count("edges", record.edges.len(), budgets.edges)?;
    ensure_count("proof records", record.proof_records.len(), budgets.proof_records)?;
    ensure_count("source-map intervals", source_map.intervals.len(), budgets.source_map_intervals)?;
    if artifact_declared_too_large(record.artifact_size_bytes, budgets.artifact_bytes) {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2400BudgetExceeded,
            "record-declared artifact size exceeds checker policy",
        ));
    }
    Ok(())
}

fn validate_metadata_binding(
    artifact: &[u8],
    metadata: &Value,
    record: &VerifiedLoweringRecord,
    source_map: &SourceArtifactMap,
) -> Result<(), CheckerError> {
    let artifact_hash = hex_encode(&ckb_blake2b256(artifact));
    if artifact_hash != record.artifact_hash || artifact.len() as u64 != record.artifact_size_bytes {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2409ArtifactIdentityMismatch,
            "artifact bytes do not match the lowering record identity",
        ));
    }
    let record_hash = canonical_hash(LOWERING_RECORD_SCHEMA, record)?;
    let source_map_hash = canonical_hash(SOURCE_MAP_SCHEMA, source_map)?;
    let comparisons = [
        ("compiler_version", json_string(metadata, &["compiler_version"]), record.compiler_version.as_str()),
        ("module", json_string(metadata, &["module"]), record.module.as_str()),
        ("edition", json_string(metadata, &["edition"]), record.edition.as_str()),
        ("target_profile.name", json_string(metadata, &["target_profile", "name"]), record.target_profile.as_str()),
        ("artifact_format", json_string(metadata, &["artifact_format"]), record.artifact_format.as_str()),
        ("artifact_hash", json_string(metadata, &["artifact_hash"]), record.artifact_hash.as_str()),
        ("source_content_hash", json_string(metadata, &["source_content_hash"]), record.source_content_hash.as_str()),
        (
            "verified_artifact.lowering_record_hash",
            json_string(metadata, &["verified_artifact", "lowering_record_hash"]),
            record_hash.as_str(),
        ),
        (
            "verified_artifact.source_map_hash",
            json_string(metadata, &["verified_artifact", "source_map_hash"]),
            source_map_hash.as_str(),
        ),
    ];
    for (field, actual, expected) in comparisons {
        if actual != Some(expected) {
            return Err(CheckerError::new(
                CheckerRejectionCode::V2410MetadataBindingMismatch,
                format!("compile metadata field '{field}' does not match lowering boundary"),
            ));
        }
    }
    if json_u64(metadata, &["artifact_size_bytes"]) != Some(record.artifact_size_bytes) {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2410MetadataBindingMismatch,
            "compile metadata artifact_size_bytes does not match lowering record",
        ));
    }
    let profile_value = metadata.get("compatibility_profile").cloned().ok_or_else(|| {
        CheckerError::new(CheckerRejectionCode::V2410MetadataBindingMismatch, "compile metadata has no compatibility_profile")
    })?;
    let profile: CompatibilityProfileIdentity = serde_json::from_value(profile_value).map_err(|error| {
        CheckerError::new(
            CheckerRejectionCode::V2410MetadataBindingMismatch,
            format!("compile metadata compatibility_profile shape is invalid: {error}"),
        )
    })?;
    if profile != record.compatibility_profile {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2410MetadataBindingMismatch,
            "compile metadata compatibility profile differs from lowering record",
        ));
    }
    if source_map.lowering_record_hash != record_hash
        || source_map.artifact_hash != record.artifact_hash
        || source_map.source_set_hash != record.source_set_hash
    {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2416SourceMapInvalid,
            "source map identity does not bind to record, artifact, and source set",
        ));
    }
    Ok(())
}

fn validate_record_graph(record: &VerifiedLoweringRecord, budgets: &CheckerBudgets) -> Result<(), CheckerError> {
    if record.entries.is_empty() || record.blocks.is_empty() || record.text_range.is_empty() {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2405ReferentialIntegrity,
            "lowering record requires at least one entry, one block, and a non-empty text range",
        ));
    }
    ensure_sorted_unique(&record.entries, |entry| entry.id.as_str(), "entry")?;
    ensure_sorted_unique(&record.blocks, |block| block.id.as_str(), "block")?;
    ensure_sorted_unique(&record.proof_records, |proof| proof.id.as_str(), "proof")?;
    if !record.edges.windows(2).all(|pair| (&pair[0].from, &pair[0].kind, &pair[0].to) < (&pair[1].from, &pair[1].kind, &pair[1].to)) {
        return Err(CheckerError::new(CheckerRejectionCode::V2404CanonicalOrder, "lowering edges are not strictly sorted and unique"));
    }

    let entries = record.entries.iter().map(|entry| (entry.id.as_str(), entry)).collect::<BTreeMap<_, _>>();
    let blocks = record.blocks.iter().map(|block| (block.id.as_str(), block)).collect::<BTreeMap<_, _>>();
    let proofs = record.proof_records.iter().map(|proof| (proof.id.as_str(), proof)).collect::<BTreeMap<_, _>>();
    for entry in &record.entries {
        let Some(block) = blocks.get(entry.entry_block.as_str()) else {
            return Err(CheckerError::new(
                CheckerRejectionCode::V2405ReferentialIntegrity,
                format!("entry '{}' references missing block '{}'", entry.id, entry.entry_block),
            ));
        };
        if block.owner_entry != entry.id {
            return Err(CheckerError::new(
                CheckerRejectionCode::V2405ReferentialIntegrity,
                format!("entry '{}' begins in block owned by '{}'", entry.id, block.owner_entry),
            ));
        }
        validate_entry_abi(entry, budgets)?;
        if !strictly_sorted(&entry.capabilities)
            || entry.capabilities.iter().any(String::is_empty)
            || !strictly_sorted(&entry.proof_ids)
        {
            return Err(CheckerError::new(
                CheckerRejectionCode::V2404CanonicalOrder,
                format!("entry '{}' has non-canonical capabilities or ProofPlan links", entry.id),
            ));
        }
        for proof_id in &entry.proof_ids {
            let Some(proof) = proofs.get(proof_id.as_str()) else {
                return Err(CheckerError::new(
                    CheckerRejectionCode::V2408ProofCoverageInvalid,
                    format!("entry '{}' references missing proof '{}'", entry.id, proof_id),
                ));
            };
            if proof.entry_id != entry.id {
                return Err(CheckerError::new(
                    CheckerRejectionCode::V2408ProofCoverageInvalid,
                    format!("proof '{}' is not owned by entry '{}'", proof_id, entry.id),
                ));
            }
        }
    }
    for proof in &record.proof_records {
        if !entries.contains_key(proof.entry_id.as_str()) || proof.obligation.is_empty() || proof.evidence_tier.is_empty() {
            return Err(CheckerError::new(
                CheckerRejectionCode::V2408ProofCoverageInvalid,
                format!("proof '{}' has an invalid owner or empty enforcement fields", proof.id),
            ));
        }
    }

    if !record
        .runtime_error_exits
        .windows(2)
        .all(|pair| (&pair[0].block_id, pair[0].code, pair[0].address) < (&pair[1].block_id, pair[1].code, pair[1].address))
    {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2404CanonicalOrder,
            "runtime-error exits are not strictly sorted and unique",
        ));
    }
    for exit in &record.runtime_error_exits {
        if exit.code <= 0
            || exit.code > 255
            || exit.name.is_empty()
            || blocks.get(exit.block_id.as_str()).is_none_or(|block| !block.range.contains(exit.address))
        {
            return Err(CheckerError::new(
                CheckerRejectionCode::V2406CfgInvalid,
                format!("runtime-error exit {} ({}) is outside its declared block", exit.code, exit.name),
            ));
        }
    }

    let mut expected_start = record.text_range.start;
    for block in &record.blocks {
        if !entries.contains_key(block.owner_entry.as_str()) {
            return Err(CheckerError::new(
                CheckerRejectionCode::V2405ReferentialIntegrity,
                format!("block '{}' has missing owner '{}'", block.id, block.owner_entry),
            ));
        }
        if block.range.start != expected_start || block.range.is_empty() || block.range.start % 4 != 0 || block.range.end % 4 != 0 {
            return Err(CheckerError::new(
                CheckerRejectionCode::V2406CfgInvalid,
                format!("block '{}' does not form aligned contiguous text coverage", block.id),
            ));
        }
        expected_start = block.range.end;
        validate_block_abi(block, entries[block.owner_entry.as_str()], &proofs, budgets)?;
    }
    if expected_start != record.text_range.end {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2406CfgInvalid,
            "lowering blocks do not cover the declared text range exactly",
        ));
    }

    let mut outgoing = BTreeMap::<&str, Vec<&LoweringEdge>>::new();
    for edge in &record.edges {
        if !blocks.contains_key(edge.from.as_str()) || !blocks.contains_key(edge.to.as_str()) {
            return Err(CheckerError::new(
                CheckerRejectionCode::V2405ReferentialIntegrity,
                format!("edge '{} -> {}' references a missing block", edge.from, edge.to),
            ));
        }
        outgoing.entry(edge.from.as_str()).or_default().push(edge);
    }
    for block in &record.blocks {
        validate_terminator_edges(block, outgoing.get(block.id.as_str()).map(Vec::as_slice).unwrap_or(&[]))?;
    }
    validate_reachability(record, &outgoing)?;
    validate_call_graph(record, &entries, &blocks, budgets.call_depth)?;
    Ok(())
}

fn validate_entry_abi(entry: &LoweringEntry, budgets: &CheckerBudgets) -> Result<(), CheckerError> {
    if entry.name.is_empty()
        || entry.return_type.is_empty()
        || entry.effect.is_empty()
        || entry.frame_size_bytes > budgets.stack_frame_bytes
        || entry.outgoing_argument_bytes > entry.frame_size_bytes
    {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2407AbiOrStackInvalid,
            format!("entry '{}' has an invalid name/frame/outgoing-argument area", entry.id),
        ));
    }
    let mut expected_index = 0u32;
    for param in &entry.params {
        if param.index != expected_index
            || param.name.is_empty()
            || param.ty.is_empty()
            || param.width_bytes == 0
            || !valid_alignment(param.alignment_bytes)
        {
            return Err(CheckerError::new(
                CheckerRejectionCode::V2407AbiOrStackInvalid,
                format!("entry '{}' has an invalid typed parameter at index {}", entry.id, param.index),
            ));
        }
        expected_index = expected_index.saturating_add(1);
    }
    Ok(())
}

fn validate_block_abi(
    block: &LoweringBlock,
    entry: &LoweringEntry,
    proofs: &BTreeMap<&str, &ProofRecord>,
    budgets: &CheckerBudgets,
) -> Result<(), CheckerError> {
    if block.frame_size_bytes != entry.frame_size_bytes
        || block.outgoing_argument_bytes != entry.outgoing_argument_bytes
        || block.effect != entry.effect
        || block.capabilities != entry.capabilities
        || block.frame_size_bytes > budgets.stack_frame_bytes
        || block.outgoing_argument_bytes > block.frame_size_bytes
    {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2407AbiOrStackInvalid,
            format!("block '{}' frame contract disagrees with owner entry", block.id),
        ));
    }
    let valid_registers = [
        "zero", "ra", "sp", "gp", "tp", "t0", "t1", "t2", "s0", "s1", "a0", "a1", "a2", "a3", "a4", "a5", "a6", "a7", "s2", "s3",
        "s4", "s5", "s6", "s7", "s8", "s9", "s10", "s11", "t3", "t4", "t5", "t6",
    ];
    if !strictly_sorted(&block.scratch_register_avoid)
        || block.scratch_register_avoid.iter().any(|register| !valid_registers.contains(&register.as_str()))
    {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2407AbiOrStackInvalid,
            format!("block '{}' has invalid scratch-register declarations", block.id),
        ));
    }
    let mut last_end = block.outgoing_argument_bytes;
    for slot in &block.stack_slots {
        if slot.name.is_empty()
            || slot.width_bytes == 0
            || !valid_alignment(slot.alignment_bytes)
            || slot.offset % slot.alignment_bytes != 0
            || slot.offset < last_end
            || slot.offset.saturating_add(slot.width_bytes) > block.frame_size_bytes
        {
            return Err(CheckerError::new(
                CheckerRejectionCode::V2407AbiOrStackInvalid,
                format!("block '{}' has overlapping, misaligned, or out-of-frame stack slot '{}'", block.id, slot.name),
            ));
        }
        last_end = slot.offset.saturating_add(slot.width_bytes);
    }
    if !strictly_sorted(&block.proof_ids)
        || block.proof_ids.iter().any(|proof_id| proofs.get(proof_id.as_str()).is_none_or(|proof| proof.entry_id != block.owner_entry))
    {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2408ProofCoverageInvalid,
            format!("block '{}' has invalid ProofPlan links", block.id),
        ));
    }
    Ok(())
}

fn validate_reachability(record: &VerifiedLoweringRecord, outgoing: &BTreeMap<&str, Vec<&LoweringEdge>>) -> Result<(), CheckerError> {
    let mut reachable = BTreeSet::new();
    let mut pending = record.entries.iter().map(|entry| entry.entry_block.as_str()).collect::<Vec<_>>();
    while let Some(block_id) = pending.pop() {
        if !reachable.insert(block_id) {
            continue;
        }
        if let Some(edges) = outgoing.get(block_id) {
            pending.extend(edges.iter().map(|edge| edge.to.as_str()));
        }
    }
    if let Some(block) = record.blocks.iter().find(|block| block.reachable != reachable.contains(block.id.as_str())) {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2406CfgInvalid,
            format!(
                "block '{}' declared reachable={} but CFG reachability is {}",
                block.id,
                block.reachable,
                reachable.contains(block.id.as_str())
            ),
        ));
    }
    Ok(())
}

fn validate_terminator_edges(block: &LoweringBlock, edges: &[&LoweringEdge]) -> Result<(), CheckerError> {
    let non_call = edges.iter().filter(|edge| edge.kind != EdgeKind::Call).map(|edge| edge.kind).collect::<Vec<_>>();
    let valid = match block.terminator {
        MachineTerminator::Fallthrough => non_call == [EdgeKind::Fallthrough],
        MachineTerminator::Jump => non_call == [EdgeKind::Jump],
        MachineTerminator::ConditionalBranch => {
            non_call == [EdgeKind::ConditionalTaken, EdgeKind::ConditionalFallthrough]
                || non_call == [EdgeKind::ConditionalFallthrough, EdgeKind::ConditionalTaken]
        }
        MachineTerminator::Return => non_call.is_empty(),
    };
    if !valid {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2406CfgInvalid,
            format!("block '{}' terminator does not match its CFG edges", block.id),
        ));
    }
    Ok(())
}

fn validate_call_graph(
    record: &VerifiedLoweringRecord,
    entries: &BTreeMap<&str, &LoweringEntry>,
    blocks: &BTreeMap<&str, &LoweringBlock>,
    max_depth: u32,
) -> Result<(), CheckerError> {
    let mut graph = BTreeMap::<&str, BTreeSet<&str>>::new();
    for edge in record.edges.iter().filter(|edge| edge.kind == EdgeKind::Call) {
        let from = blocks[edge.from.as_str()].owner_entry.as_str();
        let to = blocks[edge.to.as_str()].owner_entry.as_str();
        if from != to {
            graph.entry(from).or_default().insert(to);
        }
    }
    for root in entries.keys() {
        let mut active = BTreeSet::new();
        validate_call_depth(root, &graph, &mut active, 1, max_depth)?;
    }
    Ok(())
}

fn validate_call_depth<'a>(
    current: &'a str,
    graph: &BTreeMap<&'a str, BTreeSet<&'a str>>,
    active: &mut BTreeSet<&'a str>,
    depth: u32,
    max_depth: u32,
) -> Result<(), CheckerError> {
    if depth > max_depth {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2400BudgetExceeded,
            format!("static call depth exceeds checker budget {max_depth}"),
        ));
    }
    if !active.insert(current) {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2418RecursionPolicyInvalid,
            format!("recursive call cycle reaches entry '{current}'"),
        ));
    }
    if let Some(children) = graph.get(current) {
        for child in children {
            validate_call_depth(child, graph, active, depth.saturating_add(1), max_depth)?;
        }
    }
    active.remove(current);
    Ok(())
}

fn validate_elf_binding(artifact: &[u8], record: &VerifiedLoweringRecord, elf: &ParsedElf) -> Result<(), CheckerError> {
    if !elf.text.range().contains_range(record.text_range) {
        return Err(CheckerError::new(CheckerRejectionCode::V2412ElfSectionInvalid, "record text range is outside ELF .text"));
    }
    if record.artifact_size_bytes != artifact.len() as u64 || record.text_range.start < elf.entry {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2409ArtifactIdentityMismatch,
            "record artifact size/text identity disagrees with ELF",
        ));
    }
    Ok(())
}

fn validate_block_digests(artifact: &[u8], record: &VerifiedLoweringRecord, elf: &ParsedElf) -> Result<(), CheckerError> {
    for block in &record.blocks {
        let bytes = elf.bytes_for_range(artifact, block.range).map_err(map_elf_error)?;
        let digest = domain_hash_bytes("cellscript-machine-block-v1", bytes);
        if digest != block.byte_digest {
            return Err(CheckerError::new(
                CheckerRejectionCode::V2415BlockDigestMismatch,
                format!("machine bytes for block '{}' do not match its digest", block.id),
            ));
        }
    }
    Ok(())
}

fn validate_control_flow(record: &VerifiedLoweringRecord, elf: &ParsedElf) -> Result<(), CheckerError> {
    let blocks = record.blocks.iter().map(|block| (block.id.as_str(), block)).collect::<BTreeMap<_, _>>();
    let find_block = |address| record.blocks.iter().find(|block| block.range.contains(address));
    for flow in elf.control_flow.iter().filter(|flow| record.text_range.contains(flow.address)) {
        let Some(from) = find_block(flow.address) else {
            return Err(CheckerError::new(
                CheckerRejectionCode::V2414ControlFlowInvalid,
                format!("instruction at {:#x} is not covered by a lowering block", flow.address),
            ));
        };
        let Some(to) = find_block(flow.target) else {
            return Err(CheckerError::new(
                CheckerRejectionCode::V2414ControlFlowInvalid,
                format!("target {:#x} is outside lowering blocks", flow.target),
            ));
        };
        let allowed_kinds: &[EdgeKind] = match flow.kind {
            DecodedControlFlowKind::ConditionalBranch => {
                &[EdgeKind::ConditionalTaken, EdgeKind::ConditionalFallthrough, EdgeKind::Fallthrough]
            }
            DecodedControlFlowKind::DirectJump => &[EdgeKind::Jump, EdgeKind::Call, EdgeKind::ConditionalTaken],
        };
        let edge_exists = from.id == to.id
            || record.edges.iter().any(|edge| edge.from == from.id && edge.to == to.id && allowed_kinds.contains(&edge.kind));
        if !edge_exists || !blocks.contains_key(to.id.as_str()) {
            return Err(CheckerError::new(
                CheckerRejectionCode::V2414ControlFlowInvalid,
                format!("decoded flow '{} -> {}' is absent from the lowering CFG", from.id, to.id),
            ));
        }
    }
    Ok(())
}

fn validate_machine_terminators(record: &VerifiedLoweringRecord, elf: &ParsedElf) -> Result<(), CheckerError> {
    let instructions = elf.instructions.iter().map(|instruction| (instruction.address, instruction.word)).collect::<BTreeMap<_, _>>();
    for block in &record.blocks {
        let address = block.range.end.checked_sub(4).ok_or_else(|| {
            CheckerError::new(
                CheckerRejectionCode::V2414ControlFlowInvalid,
                format!("block '{}' is too short for a terminator", block.id),
            )
        })?;
        let word = instructions.get(&address).copied().ok_or_else(|| {
            CheckerError::new(
                CheckerRejectionCode::V2414ControlFlowInvalid,
                format!("block '{}' end does not address a decoded instruction", block.id),
            )
        })?;
        let opcode = word & 0x7f;
        let rd = (word >> 7) & 0x1f;
        let valid = match block.terminator {
            MachineTerminator::Return => word == 0x0000_8067,
            MachineTerminator::Jump => opcode == 0x6f && rd == 0,
            MachineTerminator::ConditionalBranch => opcode == 0x63 || (opcode == 0x6f && rd == 0),
            MachineTerminator::Fallthrough => word != 0x0000_8067 && !matches!(opcode, 0x63 | 0x6f),
        };
        if !valid {
            return Err(CheckerError::new(
                CheckerRejectionCode::V2414ControlFlowInvalid,
                format!("decoded final instruction of block '{}' disagrees with its terminator", block.id),
            ));
        }
    }
    Ok(())
}

fn validate_stack_discipline(record: &VerifiedLoweringRecord, elf: &ParsedElf) -> Result<(), CheckerError> {
    let blocks = record.blocks.iter().map(|block| (block.id.as_str(), block)).collect::<BTreeMap<_, _>>();
    let mut outgoing = BTreeMap::<&str, Vec<&LoweringEdge>>::new();
    for edge in &record.edges {
        outgoing.entry(edge.from.as_str()).or_default().push(edge);
    }
    let mut entry_delta = BTreeMap::<&str, i64>::new();
    let mut pending = record.entries.iter().map(|entry| (entry.entry_block.as_str(), 0_i64)).collect::<Vec<_>>();
    while let Some((block_id, incoming_delta)) = pending.pop() {
        if let Some(previous) = entry_delta.insert(block_id, incoming_delta) {
            if previous != incoming_delta {
                return Err(CheckerError::new(
                    CheckerRejectionCode::V2407AbiOrStackInvalid,
                    format!("block '{block_id}' has inconsistent incoming stack-pointer deltas {previous} and {incoming_delta}"),
                ));
            }
            continue;
        }
        let block = blocks[block_id];
        let mut delta = incoming_delta;
        for adjustment in elf.stack_adjustments.iter().filter(|adjustment| block.range.contains(adjustment.address)) {
            delta = delta.checked_add(adjustment.delta).ok_or_else(|| {
                CheckerError::new(
                    CheckerRejectionCode::V2407AbiOrStackInvalid,
                    format!("stack-pointer delta overflows in block '{block_id}'"),
                )
            })?;
            if delta > 0 || delta.unsigned_abs() > u64::from(block.frame_size_bytes) {
                return Err(CheckerError::new(
                    CheckerRejectionCode::V2407AbiOrStackInvalid,
                    format!("stack-pointer delta {delta} in block '{block_id}' exceeds declared frame {}", block.frame_size_bytes),
                ));
            }
        }
        if block.terminator == MachineTerminator::Return && delta != 0 {
            return Err(CheckerError::new(
                CheckerRejectionCode::V2407AbiOrStackInvalid,
                format!("return block '{block_id}' leaves stack-pointer delta {delta}"),
            ));
        }
        for edge in outgoing.get(block_id).into_iter().flatten() {
            pending.push((edge.to.as_str(), if edge.kind == EdgeKind::Call { 0 } else { delta }));
        }
    }
    Ok(())
}

fn validate_syscalls(record: &VerifiedLoweringRecord, elf: &ParsedElf) -> Result<(), CheckerError> {
    let actual = elf.syscall_addresses.iter().copied().filter(|address| record.text_range.contains(*address)).collect::<Vec<_>>();
    let declared = record.syscall_sites.iter().map(|site| site.address).collect::<Vec<_>>();
    if actual != declared {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2417SyscallContractInvalid,
            "declared syscall sites do not exactly match decoded ecall instructions",
        ));
    }
    let blocks = record.blocks.iter().map(|block| (block.id.as_str(), block)).collect::<BTreeMap<_, _>>();
    for site in &record.syscall_sites {
        if site.contract.is_empty()
            || site.source_domain.is_empty()
            || site.index_domain.is_empty()
            || site.buffer_limit_bytes == 0
            || !site.return_code_checked
            || blocks.get(site.block_id.as_str()).is_none_or(|block| !block.range.contains(site.address))
        {
            return Err(CheckerError::new(
                CheckerRejectionCode::V2417SyscallContractInvalid,
                format!("syscall site at {:#x} has an invalid bounded contract", site.address),
            ));
        }
    }
    Ok(())
}

fn validate_source_map(
    source_map: &SourceArtifactMap,
    record: &VerifiedLoweringRecord,
    artifact: &[u8],
    elf: &ParsedElf,
) -> Result<(), CheckerError> {
    if source_map.schema != SOURCE_MAP_SCHEMA
        || source_map.version != SOURCE_MAP_VERSION
        || source_map.module != record.module
        || source_map.text_range != record.text_range
        || source_map.coverage_claim.source_semantic_equivalence
        || !source_map.coverage_claim.mapped_instruction_ranges_only
    {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2416SourceMapInvalid,
            "source map schema, identity, or bounded claim is invalid",
        ));
    }
    let blocks = record.blocks.iter().map(|block| (block.id.as_str(), block)).collect::<BTreeMap<_, _>>();
    let entries = record.entries.iter().map(|entry| entry.id.as_str()).collect::<BTreeSet<_>>();
    let mut previous_end = None;
    let mut mapped_ranges = Vec::new();
    for interval in &source_map.intervals {
        if !safe_source_path(&interval.source_path)
            || interval.source_start > interval.source_end
            || interval.machine_range.is_empty()
            || interval.machine_range.start % 4 != 0
            || interval.machine_range.end % 4 != 0
            || previous_end.is_some_and(|end| interval.machine_range.start < end)
        {
            return Err(CheckerError::new(
                CheckerRejectionCode::V2416SourceMapInvalid,
                format!("source-map interval for block '{}' overlaps, escapes, or is malformed", interval.block_id),
            ));
        }
        let Some(block) = blocks.get(interval.block_id.as_str()) else {
            return Err(CheckerError::new(
                CheckerRejectionCode::V2416SourceMapInvalid,
                format!("source-map interval references missing block '{}'", interval.block_id),
            ));
        };
        if interval.entry_id != block.owner_entry
            || !entries.contains(interval.entry_id.as_str())
            || !block.range.contains_range(interval.machine_range)
            || interval.lowering_block_id != block.lowering_block_id
            || interval.proof_ids.iter().any(|proof| !block.proof_ids.contains(proof))
        {
            return Err(CheckerError::new(
                CheckerRejectionCode::V2416SourceMapInvalid,
                format!("source-map interval for '{}' disagrees with its lowering block", interval.block_id),
            ));
        }
        elf.bytes_for_range(artifact, interval.machine_range).map_err(map_elf_error)?;
        previous_end = Some(interval.machine_range.end);
        mapped_ranges.push(interval.machine_range);
    }
    if source_map.coverage_claim.complete_text_coverage {
        let mut expected = record.text_range.start;
        for range in mapped_ranges {
            if range.start != expected {
                return Err(CheckerError::new(
                    CheckerRejectionCode::V2416SourceMapInvalid,
                    "source map claims complete text coverage but contains a gap",
                ));
            }
            expected = range.end;
        }
        if expected != record.text_range.end {
            return Err(CheckerError::new(
                CheckerRejectionCode::V2416SourceMapInvalid,
                "source map claims complete text coverage but does not reach text end",
            ));
        }
    }
    Ok(())
}

fn safe_source_path(path: &str) -> bool {
    if path == "<memory>" {
        return true;
    }
    if path.is_empty() || path.starts_with('/') || path.starts_with('\\') || path.contains('\\') {
        return false;
    }
    if path.len() >= 2 && path.as_bytes()[1] == b':' {
        return false;
    }
    path.split('/').all(|component| !matches!(component, "" | "." | ".."))
}

fn ensure_canonical<T: Serialize>(label: &str, input: &[u8], value: &T) -> Result<(), CheckerError> {
    let canonical = canonical_bytes(value)?;
    if canonical != input {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2402NonCanonicalJson,
            format!("{label} is not byte-for-byte canonical JSON"),
        ));
    }
    Ok(())
}

fn ensure_byte_budget(label: &str, actual: usize, limit: u64) -> Result<(), CheckerError> {
    if actual as u64 > limit {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2400BudgetExceeded,
            format!("{label} bytes {actual} exceed budget {limit}"),
        ));
    }
    Ok(())
}

fn ensure_count(label: &str, actual: usize, limit: u32) -> Result<(), CheckerError> {
    if actual as u64 > u64::from(limit) {
        return Err(CheckerError::new(
            CheckerRejectionCode::V2400BudgetExceeded,
            format!("{label} count {actual} exceeds budget {limit}"),
        ));
    }
    Ok(())
}

fn ensure_sorted_unique<'a, T, F>(values: &'a [T], key: F, label: &str) -> Result<(), CheckerError>
where
    F: Fn(&'a T) -> &'a str,
{
    if values.windows(2).all(|pair| key(&pair[0]) < key(&pair[1])) {
        Ok(())
    } else {
        Err(CheckerError::new(
            CheckerRejectionCode::V2404CanonicalOrder,
            format!("{label} identifiers are not strictly sorted and unique"),
        ))
    }
}

fn strictly_sorted<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_alignment(value: u32) -> bool {
    value.is_power_of_two() && value <= 16
}

fn artifact_declared_too_large(actual: u64, limit: u64) -> bool {
    actual > limit
}

fn json_string<'a>(root: &'a Value, path: &[&str]) -> Option<&'a str> {
    path.iter().try_fold(root, |value, key| value.get(*key)).and_then(Value::as_str)
}

fn json_u64(root: &Value, path: &[&str]) -> Option<u64> {
    path.iter().try_fold(root, |value, key| value.get(*key)).and_then(Value::as_u64)
}

pub fn domain_hash_bytes(domain: &str, bytes: &[u8]) -> String {
    let mut material = Vec::with_capacity(domain.len() + 1 + bytes.len());
    material.extend_from_slice(domain.as_bytes());
    material.push(0);
    material.extend_from_slice(bytes);
    hex_encode(&ckb_blake2b256(&material))
}

fn map_elf_error(error: ElfParseError) -> CheckerError {
    let code = match error.kind {
        ElfErrorKind::BudgetExceeded => CheckerRejectionCode::V2400BudgetExceeded,
        ElfErrorKind::InvalidSection | ElfErrorKind::ProhibitedLinkState | ElfErrorKind::MissingText => {
            CheckerRejectionCode::V2412ElfSectionInvalid
        }
        ElfErrorKind::InvalidInstruction => CheckerRejectionCode::V2413InstructionInvalid,
        ElfErrorKind::InvalidBranchTarget => CheckerRejectionCode::V2414ControlFlowInvalid,
        _ => CheckerRejectionCode::V2411ElfFormatInvalid,
    };
    CheckerError::new(code, error.message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_parser_rejects_whitespace_and_unknown_fields() {
        let budgets = CheckerBudgets::default();
        let unknown = br#"{"schema":"cellscript-source-artifact-map-v1","version":1,"module":"m","artifact_hash":"h","lowering_record_hash":"r","source_set_hash":"s","text_range":{"start":1,"end":2},"intervals":[],"coverage_claim":{"mapped_instruction_ranges_only":true,"complete_text_coverage":false,"source_semantic_equivalence":false},"unknown":true}"#;
        assert_eq!(parse_source_map(unknown, &budgets).unwrap_err().code, CheckerRejectionCode::V2401MalformedJson);

        let map = SourceArtifactMap {
            schema: SOURCE_MAP_SCHEMA.to_string(),
            version: SOURCE_MAP_VERSION,
            module: "m".to_string(),
            artifact_hash: "h".to_string(),
            lowering_record_hash: "r".to_string(),
            source_set_hash: "s".to_string(),
            text_range: MachineRange { start: 1, end: 2 },
            intervals: Vec::new(),
            coverage_claim: SourceMapCoverageClaim {
                mapped_instruction_ranges_only: true,
                complete_text_coverage: false,
                source_semantic_equivalence: false,
            },
        };
        let mut pretty = serde_json::to_vec_pretty(&map).unwrap();
        pretty.push(b'\n');
        assert_eq!(parse_source_map(&pretty, &budgets).unwrap_err().code, CheckerRejectionCode::V2402NonCanonicalJson);
    }

    #[test]
    fn checker_error_diagnostics_are_utf8_bounded() {
        let error = CheckerError::new(CheckerRejectionCode::V2401MalformedJson, "边界".repeat(100)).bounded(10);
        assert!(error.message.len() <= 10);
        assert!(std::str::from_utf8(error.message.as_bytes()).is_ok());
    }

    #[test]
    fn malformed_corpus_is_bounded_and_never_panics() {
        let budgets = CheckerBudgets {
            artifact_bytes: 4_096,
            record_bytes: 4_096,
            source_map_bytes: 4_096,
            diagnostic_bytes: 64,
            ..CheckerBudgets::default()
        };
        let corpus = [
            Vec::new(),
            vec![0xff],
            b"{".to_vec(),
            vec![b'{'; 4_097],
            (0..4_096).map(|index| (index % 251) as u8).collect::<Vec<_>>(),
        ];
        for bytes in corpus {
            let outcome = std::panic::catch_unwind(|| check_bundle(&bytes, &bytes, &bytes, &bytes, &budgets));
            let error = outcome.expect("checker must not panic on malformed bounded corpus").unwrap_err();
            assert!(error.message.len() <= budgets.diagnostic_bytes as usize);
        }
    }

    #[test]
    fn source_paths_are_confined() {
        assert!(safe_source_path("src/main.cell"));
        assert!(safe_source_path("<memory>"));
        assert!(!safe_source_path("../main.cell"));
        assert!(!safe_source_path("/tmp/main.cell"));
        assert!(!safe_source_path("C:/main.cell"));
    }
}
