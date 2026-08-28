use serde::{Deserialize, Serialize};

pub const LOWERING_RECORD_SCHEMA: &str = "cellscript-verified-lowering-record-v4";
pub const TYPED_SEMANTICS_SCHEMA: &str = "cellscript-typed-semantics-v3";
pub const SOURCE_MAP_SCHEMA: &str = "cellscript-source-artifact-map-v1";
pub const CHECKER_POLICY_SCHEMA: &str = "cellscript-artifact-checker-policy-v1";
pub const CHECKER_REPORT_SCHEMA: &str = "cellscript-artifact-checker-report-v1";
pub const LOWERING_RECORD_VERSION: u32 = 4;
pub const TYPED_SEMANTICS_VERSION: u32 = 3;
pub const SOURCE_MAP_VERSION: u32 = 1;
pub const CHECKER_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompatibilityProfileIdentity {
    pub schema: String,
    pub id: String,
    pub edition: String,
    pub source_semantics: String,
    pub target_profile: String,
    pub primitive_assurance: String,
    pub metadata_schema_version: u32,
    pub source_metadata_schema_version: u32,
    pub artifact_metadata_schema_version: u32,
    pub constraints_metadata_schema_version: u32,
    pub entry_witness_payload_abi: String,
    pub entry_witness_placement_abi: String,
    pub entry_witness_placement_field: String,
    pub entry_witness_placement_source: String,
    pub raw_entry_witness_payload_compatible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedLoweringRecord {
    pub schema: String,
    pub version: u32,
    pub compiler_version: String,
    pub module: String,
    pub edition: String,
    pub target_profile: String,
    pub compatibility_profile: CompatibilityProfileIdentity,
    pub compatibility_profile_hash: String,
    pub source_set_hash: String,
    pub source_content_hash: String,
    pub artifact_format: String,
    pub artifact_hash: String,
    pub artifact_size_bytes: u64,
    pub typed_semantics: TypedSemanticRecord,
    pub typed_semantics_hash: String,
    pub text_range: MachineRange,
    pub entries: Vec<LoweringEntry>,
    pub blocks: Vec<LoweringBlock>,
    pub edges: Vec<LoweringEdge>,
    pub proof_records: Vec<ProofRecord>,
    pub syscall_sites: Vec<SyscallSite>,
    pub runtime_error_exits: Vec<RuntimeErrorExit>,
    pub limits: DeclaredLimits,
    pub claim: VerificationClaim,
}

impl VerifiedLoweringRecord {
    pub fn canonicalize(&mut self) {
        self.entries.sort_by(|a, b| a.id.cmp(&b.id));
        self.typed_semantics.canonicalize();
        for entry in &mut self.entries {
            entry.params.sort_by_key(|param| param.index);
            entry.proof_ids.sort();
            entry.proof_ids.dedup();
            entry.capabilities.sort();
            entry.capabilities.dedup();
            entry.typed_blocks.sort_by_key(|block| block.id);
            for block in &mut entry.typed_blocks {
                block.machine_block_ids.sort();
                block.machine_block_ids.dedup();
            }
        }
        self.blocks.sort_by(|a, b| a.id.cmp(&b.id));
        for block in &mut self.blocks {
            block.stack_slots.sort_by(|a, b| a.offset.cmp(&b.offset).then(a.name.cmp(&b.name)));
            block.scratch_register_avoid.sort();
            block.scratch_register_avoid.dedup();
            block.proof_ids.sort();
            block.proof_ids.dedup();
            block.capabilities.sort();
            block.capabilities.dedup();
        }
        self.edges.sort_by(|a, b| (&a.from, &a.kind, &a.to).cmp(&(&b.from, &b.kind, &b.to)));
        self.edges.dedup_by(|a, b| a.from == b.from && a.kind == b.kind && a.to == b.to);
        self.proof_records.sort_by(|a, b| a.id.cmp(&b.id));
        self.syscall_sites.sort_by(|a, b| (a.address, &a.block_id).cmp(&(b.address, &b.block_id)));
        self.runtime_error_exits.sort_by(|a, b| (&a.block_id, a.code, a.address).cmp(&(&b.block_id, b.code, b.address)));
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedSemanticRecord {
    pub schema: String,
    pub version: u32,
    pub module: String,
    pub interface_hash: String,
    pub types: Vec<TypedSemanticType>,
    pub entries: Vec<TypedSemanticEntry>,
    pub instantiations: Vec<TypedSemanticInstantiation>,
}

impl TypedSemanticRecord {
    pub fn canonicalize(&mut self) {
        self.types.sort_by(|left, right| left.name.cmp(&right.name));
        for ty in &mut self.types {
            ty.fields.sort_by(|left, right| left.offset.cmp(&right.offset).then(left.name.cmp(&right.name)));
            ty.variants.sort_by(|left, right| left.tag.cmp(&right.tag).then(left.name.cmp(&right.name)));
            for variant in &mut ty.variants {
                variant.fields.sort_by_key(|field| field.index);
            }
            ty.capabilities.sort();
            ty.capabilities.dedup();
        }
        self.entries.sort_by(|left, right| left.id.cmp(&right.id));
        for entry in &mut self.entries {
            entry.locals.sort_by_key(|local| local.id);
            entry.blocks.sort_by_key(|block| block.id);
            for block in &mut entry.blocks {
                block.successors.sort();
                block.successors.dedup();
            }
            entry
                .borrows
                .sort_by(|left, right| (&left.root, &left.path, &left.binding).cmp(&(&right.root, &right.path, &right.binding)));
            entry.ownership.sort_by(|left, right| (&left.binding, &left.operation).cmp(&(&right.binding, &right.operation)));
            entry.obligations.sort();
            entry.obligations.dedup();
        }
        self.instantiations.sort_by(|left, right| left.identity.cmp(&right.identity));
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedSemanticType {
    pub name: String,
    pub kind: String,
    pub encoded_size: Option<u32>,
    pub fields: Vec<TypedSemanticField>,
    pub tag_width_bytes: Option<u32>,
    pub variants: Vec<TypedSemanticVariant>,
    pub capabilities: Vec<String>,
    pub identity_policy: String,
    pub layout_hash: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedSemanticField {
    pub name: String,
    pub ty: String,
    pub offset: u32,
    pub width_bytes: Option<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedSemanticVariant {
    pub name: String,
    pub tag: u32,
    pub payload_width_bytes: u32,
    pub fields: Vec<TypedSemanticVariantField>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedSemanticVariantField {
    pub index: u32,
    pub ty: String,
    pub offset: u32,
    pub width_bytes: u32,
    pub linear: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedSemanticEntry {
    pub id: String,
    pub kind: String,
    pub name: String,
    pub params: Vec<TypedSemanticParam>,
    pub return_type: String,
    pub effect: String,
    pub entry_block: u32,
    pub locals: Vec<TypedSemanticLocal>,
    pub blocks: Vec<TypedSemanticBlock>,
    pub borrows: Vec<TypedSemanticBorrow>,
    pub ownership: Vec<TypedSemanticOwnership>,
    pub obligations: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedSemanticParam {
    pub index: u32,
    pub binding_id: u32,
    pub name: String,
    pub ty: String,
    pub source: String,
    pub mutable: bool,
    pub reference: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedSemanticLocal {
    pub id: u32,
    pub source_id: u64,
    pub name: String,
    pub ty: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedSemanticBlock {
    pub id: u32,
    pub operations: Vec<TypedSemanticOperation>,
    pub successors: Vec<u32>,
    pub terminator: String,
    pub runtime_error: Option<TypedSemanticRuntimeError>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedSemanticRuntimeError {
    pub code: u64,
    pub name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedSemanticOperation {
    pub index: u32,
    pub opcode: String,
    pub destinations: Vec<u32>,
    pub operands: Vec<TypedSemanticOperand>,
    pub detail: TypedSemanticOperationDetail,
    pub call: Option<TypedSemanticCall>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedSemanticOperand {
    pub local: Option<u32>,
    pub ty: String,
    pub constant: Option<TypedSemanticConstant>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum TypedSemanticOperationDetail {
    #[default]
    None,
    Constant {
        value: TypedSemanticConstant,
    },
    Binding {
        name: String,
    },
    BinaryOperator {
        operator: String,
    },
    UnaryOperator {
        operator: String,
    },
    Field {
        name: String,
    },
    Collection {
        declared_type: String,
    },
    Reference {
        declared_type: String,
    },
    EnumConstruct {
        enum_name: String,
        variant: String,
    },
    EnumTag {
        enum_name: String,
    },
    EnumPayload {
        enum_name: String,
        variant: String,
        field_index: u32,
    },
    Create {
        pattern: TypedSemanticCreatePattern,
    },
    Destroy {
        policy: String,
    },
    CreateUnique {
        pattern: TypedSemanticCreatePattern,
        identity: String,
    },
    ReplaceUnique {
        pattern: TypedSemanticCreatePattern,
        identity: String,
    },
    CellMetadata {
        field: String,
    },
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedSemanticCreatePattern {
    pub operation: String,
    pub ty: String,
    pub binding: String,
    pub field_names: Vec<String>,
    pub has_lock: bool,
    pub identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "kebab-case", deny_unknown_fields)]
pub enum TypedSemanticConstant {
    Unit,
    U8(String),
    U16(String),
    U32(String),
    U64(String),
    U128(String),
    Bool(bool),
    Address(String),
    Hash(String),
    Array(Vec<TypedSemanticConstant>),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedSemanticCall {
    pub target: String,
    pub params: Vec<String>,
    pub return_type: String,
    pub effect: String,
    pub contract: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedSemanticBorrow {
    pub root: String,
    pub path: Vec<String>,
    pub binding: String,
    pub root_type: String,
    pub view_type: String,
    pub start_block: u32,
    pub start_operation: u32,
    pub end_block: Option<u32>,
    pub end_operation: Option<u32>,
    pub escapes: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedSemanticOwnership {
    pub binding: String,
    pub ty: String,
    pub operation: String,
    pub initial_state: String,
    pub final_state: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedSemanticInstantiation {
    pub kind: String,
    pub module: String,
    pub template: String,
    pub concrete_name: String,
    pub identity: String,
    pub type_arguments: Vec<String>,
    pub value_ability_registry_version: u32,
    pub constraints_verified: bool,
    pub fixed_layout_required: bool,
    pub cell_backed_layout_rejected: bool,
    pub identity_includes_phantom_arguments: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoweringEntry {
    pub id: String,
    pub kind: EntryKind,
    pub name: String,
    pub entry_block: String,
    pub params: Vec<TypedParameter>,
    pub return_type: String,
    pub effect: String,
    pub capabilities: Vec<String>,
    pub proof_ids: Vec<String>,
    pub frame_size_bytes: u32,
    pub outgoing_argument_bytes: u32,
    pub typed_blocks: Vec<TypedBlockBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedBlockBinding {
    pub id: u32,
    pub hash: String,
    pub machine_block_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EntryKind {
    Action,
    Lock,
    Helper,
    Runtime,
    Wrapper,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypedParameter {
    pub index: u32,
    pub name: String,
    pub ty: String,
    pub storage: StorageClass,
    pub width_bytes: u32,
    pub alignment_bytes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StorageClass {
    Scalar,
    FixedBytes,
    SchemaPointer,
    Reference,
    Aggregate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoweringBlock {
    pub id: String,
    pub owner_entry: String,
    pub reachable: bool,
    pub lowering_block_id: Option<u32>,
    pub typed_block_hash: Option<String>,
    pub machine_label: Option<String>,
    pub terminator: MachineTerminator,
    pub range: MachineRange,
    pub byte_digest: String,
    pub frame_size_bytes: u32,
    pub outgoing_argument_bytes: u32,
    pub stack_slots: Vec<StackSlot>,
    pub scratch_register_avoid: Vec<String>,
    pub effect: String,
    pub capabilities: Vec<String>,
    pub proof_ids: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MachineTerminator {
    Fallthrough,
    Jump,
    ConditionalBranch,
    Return,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MachineRange {
    pub start: u64,
    pub end: u64,
}

impl MachineRange {
    pub fn len(self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty(self) -> bool {
        self.start == self.end
    }

    pub fn contains(self, address: u64) -> bool {
        self.start <= address && address < self.end
    }

    pub fn contains_range(self, other: Self) -> bool {
        self.start <= other.start && other.end <= self.end
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StackSlot {
    pub name: String,
    pub offset: u32,
    pub width_bytes: u32,
    pub alignment_bytes: u32,
    pub kind: StorageClass,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LoweringEdge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EdgeKind {
    Fallthrough,
    Jump,
    ConditionalTaken,
    ConditionalFallthrough,
    Call,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProofRecord {
    pub id: String,
    pub entry_id: String,
    pub obligation: String,
    pub evidence_tier: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SyscallSite {
    pub block_id: String,
    pub address: u64,
    pub syscall_number: Option<u64>,
    pub contract: String,
    pub source_domain: String,
    pub index_domain: String,
    pub return_code_checked: bool,
    pub buffer_limit_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeErrorExit {
    pub block_id: String,
    pub address: u64,
    pub code: i32,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredLimits {
    pub artifact_bytes: u64,
    pub record_bytes: u64,
    pub source_map_bytes: u64,
    pub entries: u32,
    pub blocks: u32,
    pub edges: u32,
    pub instructions: u64,
    pub call_depth: u32,
    pub stack_frame_bytes: u32,
    pub proof_records: u32,
    pub source_map_intervals: u32,
    pub diagnostic_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationClaim {
    pub lowering_record: String,
    pub machine_code: String,
    pub semantic_equivalence: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceArtifactMap {
    pub schema: String,
    pub version: u32,
    pub module: String,
    pub artifact_hash: String,
    pub lowering_record_hash: String,
    pub source_set_hash: String,
    pub text_range: MachineRange,
    pub intervals: Vec<SourceMapInterval>,
    pub coverage_claim: SourceMapCoverageClaim,
}

impl SourceArtifactMap {
    pub fn canonicalize(&mut self) {
        self.intervals.sort_by(|a, b| {
            (a.machine_range.start, a.machine_range.end, &a.block_id, &a.source_path, a.source_start, a.source_end).cmp(&(
                b.machine_range.start,
                b.machine_range.end,
                &b.block_id,
                &b.source_path,
                b.source_start,
                b.source_end,
            ))
        });
        for interval in &mut self.intervals {
            interval.proof_ids.sort();
            interval.proof_ids.dedup();
            interval.runtime_error_codes.sort();
            interval.runtime_error_codes.dedup();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceMapInterval {
    pub source_path: String,
    pub source_start: u32,
    pub source_end: u32,
    pub entry_id: String,
    pub block_id: String,
    pub lowering_block_id: Option<u32>,
    pub machine_range: MachineRange,
    pub proof_ids: Vec<String>,
    pub runtime_error_codes: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceMapCoverageClaim {
    pub mapped_instruction_ranges_only: bool,
    pub complete_text_coverage: bool,
    pub source_semantic_equivalence: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckerBudgets {
    pub schema: String,
    pub artifact_bytes: u64,
    pub record_bytes: u64,
    pub source_map_bytes: u64,
    pub entries: u32,
    pub blocks: u32,
    pub edges: u32,
    pub instructions: u64,
    pub call_depth: u32,
    pub stack_frame_bytes: u32,
    pub proof_records: u32,
    pub source_map_intervals: u32,
    pub diagnostic_bytes: u32,
}

impl Default for CheckerBudgets {
    fn default() -> Self {
        Self {
            schema: CHECKER_POLICY_SCHEMA.to_string(),
            artifact_bytes: 4 * 1024 * 1024,
            record_bytes: 4 * 1024 * 1024,
            source_map_bytes: 4 * 1024 * 1024,
            entries: 2_048,
            blocks: 65_536,
            edges: 262_144,
            instructions: 1_048_576,
            call_depth: 256,
            stack_frame_bytes: 1024 * 1024,
            proof_records: 65_536,
            source_map_intervals: 65_536,
            diagnostic_bytes: 16 * 1024,
        }
    }
}

impl CheckerBudgets {
    pub fn as_declared_limits(&self) -> DeclaredLimits {
        DeclaredLimits {
            artifact_bytes: self.artifact_bytes,
            record_bytes: self.record_bytes,
            source_map_bytes: self.source_map_bytes,
            entries: self.entries,
            blocks: self.blocks,
            edges: self.edges,
            instructions: self.instructions,
            call_depth: self.call_depth,
            stack_frame_bytes: self.stack_frame_bytes,
            proof_records: self.proof_records,
            source_map_intervals: self.source_map_intervals,
            diagnostic_bytes: self.diagnostic_bytes,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedArtifactMetadata {
    pub boundary_schema: String,
    pub state: VerifiedArtifactState,
    pub checker_name: String,
    pub checker_version: String,
    pub checker_policy_schema: String,
    pub lowering_record_schema: String,
    pub lowering_record_hash: Option<String>,
    pub source_map_schema: String,
    pub source_map_hash: Option<String>,
    pub claim: String,
}

impl Default for VerifiedArtifactMetadata {
    fn default() -> Self {
        Self {
            boundary_schema: "cellscript-verified-artifact-boundary-v1".to_string(),
            state: VerifiedArtifactState::NotEmittedNonElf,
            checker_name: "cellscript-artifact-checker".to_string(),
            checker_version: CHECKER_VERSION.to_string(),
            checker_policy_schema: CHECKER_POLICY_SCHEMA.to_string(),
            lowering_record_schema: LOWERING_RECORD_SCHEMA.to_string(),
            lowering_record_hash: None,
            source_map_schema: SOURCE_MAP_SCHEMA.to_string(),
            source_map_hash: None,
            claim: "unverified".to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VerifiedArtifactState {
    Emitted,
    NotEmittedNonElf,
}
