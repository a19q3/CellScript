//! Real compiler/checker boundary mutations. Rebinding sidecar and semantic
//! identities must not turn a contradictory builder/policy projection valid.
//! These tests do not establish arbitrary machine dispatch equivalence.

use cellscript::artifact::{ArtifactAction, ArtifactContext, ArtifactDeclaration, ArtifactDispatch};
use cellscript::{
    compile_path_with_executable_surface_policy, CellScriptEdition, CompileEntryScope, CompileOptions, ExecutableSurfacePolicy,
};
use cellscript_artifact_checker::{
    canonical_hash, check_bundle_values, CheckerBudgets, CheckerError, CheckerRejectionCode, EntryDispatchContract,
    PolicyWitnessContract, SourceArtifactMap, ValueProvenance, VerifiedLoweringRecord, LOWERING_RECORD_SCHEMA, SOURCE_MAP_SCHEMA,
    TYPED_SEMANTICS_SCHEMA,
};
use serde_json::Value;

const SOURCE: &str = r#"
module policy_artifact_checker
resource Token has store, consume { amount: u64 }
action check_z() { verification require true }
action check_a() { verification require true }
action mint(witness amount: u64, witness recipient: Address) {
    verification
    require amount > 0
    create Token { amount: amount } with_lock(recipient)
}
action burn(input token: Token) { verification consume token }
"#;

fn declaration() -> ArtifactDeclaration {
    ArtifactDeclaration {
        name: "token-policy".to_string(),
        context: ArtifactContext::TypeGroup { resource: "Token".to_string() },
        dispatch: ArtifactDispatch::PolicyWitnessV1,
        actions: vec![ArtifactAction { tag: 40, action: "burn".to_string() }, ArtifactAction { tag: 10, action: "mint".to_string() }],
        common_checks: vec!["check_z".to_string(), "check_a".to_string()],
    }
}

#[derive(Clone)]
struct Fixture {
    artifact: Vec<u8>,
    metadata: Value,
    record: VerifiedLoweringRecord,
    source_map: SourceArtifactMap,
}

impl Fixture {
    fn new(edition: CellScriptEdition) -> Self {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("main.cell");
        std::fs::write(&source, SOURCE).unwrap();
        let result = compile_path_with_executable_surface_policy(
            source.to_str().unwrap(),
            CompileOptions { edition, target: Some("riscv64-elf".to_string()), ..CompileOptions::default() },
            Some(CompileEntryScope::Artifact(declaration())),
            ExecutableSurfacePolicy::DenyFailClosed,
        )
        .unwrap();
        let fixture = Self {
            artifact: result.artifact_bytes,
            metadata: serde_json::to_value(result.metadata).unwrap(),
            record: result.verified_lowering_record.unwrap(),
            source_map: result.source_artifact_map.unwrap(),
        };
        fixture.check().unwrap();
        fixture
    }

    fn check(&self) -> Result<(), CheckerError> {
        check_bundle_values(&self.artifact, &self.metadata, &self.record, &self.source_map, &CheckerBudgets::default()).map(|_| ())
    }

    fn policy_mut(&mut self) -> &mut PolicyWitnessContract {
        let EntryDispatchContract::PolicyWitnessV1(policy) = &mut self.record.typed_semantics.foundation.entry_contract.dispatch
        else {
            panic!("expected policy dispatch");
        };
        policy
    }

    fn param_mut(&mut self, action: &str, index: usize) -> &mut Value {
        &mut self.metadata["actions"].as_array_mut().unwrap().iter_mut().find(|entry| entry["name"] == action).unwrap()["params"]
            [index]
    }

    fn rebind_sidecars(&mut self) {
        let record_hash = canonical_hash(LOWERING_RECORD_SCHEMA, &self.record).unwrap();
        self.source_map.lowering_record_hash = record_hash.clone();
        let source_map_hash = canonical_hash(SOURCE_MAP_SCHEMA, &self.source_map).unwrap();
        let verified_bundle_id = canonical_hash(
            "cellscript-verified-bundle-id-v1",
            &(
                self.record.artifact_hash.as_str(),
                self.record.typed_semantics_hash.as_str(),
                self.record.compatibility_profile_hash.as_str(),
                record_hash.as_str(),
                source_map_hash.as_str(),
                self.source_map.source_digest.as_str(),
            ),
        )
        .unwrap();
        self.metadata["verified_artifact"]["lowering_record_hash"] = record_hash.into();
        self.metadata["verified_artifact"]["source_map_hash"] = source_map_hash.into();
        self.metadata["verified_artifact"]["verified_bundle_id"] = verified_bundle_id.into();
    }

    fn rebind_policy_identity(&mut self) {
        let typed = &mut self.record.typed_semantics;
        typed.canonicalize();
        let foundation = &mut typed.foundation;
        let contract = &mut foundation.entry_contract;
        let previous_node = contract.semantic_node_id.clone();
        contract.semantic_node_id = canonical_hash(
            "cellscript-semantic-node-entry-contract-v2",
            &(
                contract.script_role.as_str(),
                contract.trigger.as_str(),
                contract.exact_entry.as_str(),
                &contract.dispatch,
                contract.entry_payload_abi.as_str(),
                contract.witness_placement_abi.as_str(),
                contract.witness_placement_field.as_str(),
                contract.witness_placement_source.as_str(),
            ),
        )
        .unwrap();
        for mapping in &mut self.source_map.semantic_mappings {
            if mapping.semantic_node_id == previous_node {
                mapping.semantic_node_id = contract.semantic_node_id.clone();
            }
        }
        let roots = foundation
            .provenance
            .nodes
            .iter()
            .filter(|node| !matches!(node.provenance, ValueProvenance::Derived { .. }))
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>();
        foundation.identities.core_semantic_id = canonical_hash(
            "cellscript-core-semantic-id-v2",
            &(
                typed.failure_semantics,
                &typed.types,
                &foundation.roles,
                &foundation.dispositions,
                &foundation.claims,
                &foundation.legacy_nodes,
            ),
        )
        .unwrap();
        foundation.identities.entry_contract_id = canonical_hash(
            "cellscript-entry-contract-id-v1",
            &(
                foundation.identities.core_semantic_id.as_str(),
                &foundation.entry_contract,
                roots,
                foundation.entry_contract.entry_payload_abi.as_str(),
                foundation.entry_contract.witness_placement_abi.as_str(),
            ),
        )
        .unwrap();
        foundation.identities.artifact_contract_id = canonical_hash(
            "cellscript-artifact-contract-id-v1",
            &(foundation.identities.entry_contract_id.as_str(), &foundation.artifact_contract),
        )
        .unwrap();
        self.source_map.canonicalize();
        self.metadata["verified_artifact"]["core_semantic_id"] = foundation.identities.core_semantic_id.clone().into();
        self.metadata["verified_artifact"]["entry_contract_id"] = foundation.identities.entry_contract_id.clone().into();
        self.metadata["verified_artifact"]["artifact_contract_id"] = foundation.identities.artifact_contract_id.clone().into();
        self.record.typed_semantics_hash = canonical_hash(TYPED_SEMANTICS_SCHEMA, typed).unwrap();
        self.metadata["typed_semantics"] = serde_json::to_value(typed).unwrap();
        self.metadata["typed_semantics_hash"] = self.record.typed_semantics_hash.clone().into();
        self.rebind_sidecars();
    }
}

#[test]
fn real_policy_bundle_and_unchanged_identity_rebinding_are_valid_in_both_editions() {
    for edition in [CellScriptEdition::Edition2026, CellScriptEdition::Edition2027] {
        let mut fixture = Fixture::new(edition);
        let original_record = canonical_hash(LOWERING_RECORD_SCHEMA, &fixture.record).unwrap();
        fixture.rebind_policy_identity();
        assert_eq!(canonical_hash(LOWERING_RECORD_SCHEMA, &fixture.record).unwrap(), original_record);
        fixture.check().unwrap();
    }
}

#[test]
fn raw_builder_param_mutations_reject_despite_rebound_outer_identities() {
    let fixture = Fixture::new(CellScriptEdition::Edition2027);
    for mutation in [
        "name",
        "type",
        "source",
        "mut",
        "ref",
        "cell-skip",
        "scalar-skip",
        "lock-args",
        "schema",
        "fixed-width",
        "fixed-flag",
        "hash-flag",
        "bounded",
    ] {
        let mut changed = fixture.clone();
        match mutation {
            "name" => changed.param_mut("mint", 0)["name"] = "other".into(),
            "type" => changed.param_mut("mint", 0)["ty"] = "u128".into(),
            "source" => changed.param_mut("mint", 0)["source"] = "lock_args".into(),
            "mut" => changed.param_mut("mint", 0)["is_mut"] = true.into(),
            "ref" => changed.param_mut("mint", 0)["is_ref"] = true.into(),
            "cell-skip" => changed.param_mut("burn", 0)["cell_bound_abi"] = false.into(),
            "scalar-skip" => changed.param_mut("mint", 0)["cell_bound_abi"] = true.into(),
            "lock-args" => changed.param_mut("mint", 0)["lock_args_data_source"] = true.into(),
            "schema" => changed.param_mut("mint", 0)["schema_pointer_abi"] = true.into(),
            "fixed-width" => changed.param_mut("mint", 1)["fixed_byte_len"] = 31.into(),
            "fixed-flag" => changed.param_mut("mint", 1)["fixed_byte_length_abi"] = false.into(),
            "hash-flag" => changed.param_mut("burn", 0)["type_hash_pointer_abi"] = true.into(),
            "bounded" => changed.param_mut("burn", 0)["bounded_runtime_contract"] = "type-group-inputs-v1".into(),
            _ => unreachable!(),
        }
        changed.rebind_sidecars();
        let error = changed.check().unwrap_err();
        assert_eq!(error.code, CheckerRejectionCode::V2410MetadataBindingMismatch, "{mutation}: {error}");
        assert!(error.message.contains("policy builder parameter"), "{mutation}: {error}");
    }
}

#[test]
fn raw_policy_declaration_and_outer_abi_mutations_reject_after_rebinding() {
    let fixture = Fixture::new(CellScriptEdition::Edition2027);
    for mutation in
        ["tag", "action", "common-order", "resource", "name", "records", "bytes", "payload", "placement", "field", "source", "missing"]
    {
        let mut changed = fixture.clone();
        let policy = &mut changed.metadata["runtime"]["policy_artifact"];
        match mutation {
            "tag" => policy["declaration"]["actions"][0]["tag"] = 11.into(),
            "action" => policy["declaration"]["actions"][0]["action"] = "burn".into(),
            "common-order" => policy["declaration"]["common_checks"].as_array_mut().unwrap().swap(0, 1),
            "resource" => policy["declaration"]["context"]["resource"] = "OtherToken".into(),
            "name" => policy["declaration"]["name"] = "other-policy".into(),
            "records" => policy["max_records"] = 9.into(),
            "bytes" => policy["max_witness_bytes"] = 4097.into(),
            "payload" => policy["payload_abi"] = "cellscript-entry-witness-v1".into(),
            "placement" => policy["placement_abi"] = "raw".into(),
            "field" => policy["placement_field"] = "lock".into(),
            "source" => policy["placement_source"] = "input[0]".into(),
            "missing" => {
                changed.metadata["runtime"].as_object_mut().unwrap().remove("policy_artifact");
            }
            _ => unreachable!(),
        }
        changed.rebind_sidecars();
        let error = changed.check().unwrap_err();
        assert_eq!(error.code, CheckerRejectionCode::V2410MetadataBindingMismatch, "{mutation}: {error}");
        assert!(error.message.contains("runtime.policy_artifact"), "{mutation}: {error}");
    }
}

#[test]
fn typed_policy_counts_payload_identity_and_selector_require_concrete_evidence() {
    let fixture = Fixture::new(CellScriptEdition::Edition2027);
    for mutation in ["input-count", "output-count", "payload-schema", "selector-content", "selector-label", "unknown", "wrapper"] {
        let mut changed = fixture.clone();
        match mutation {
            "input-count" => changed.policy_mut().variants[0].input_count = 1,
            "output-count" => changed.policy_mut().variants[0].output_count = 2,
            "payload-schema" => changed.policy_mut().variants[0].payload_schema_hash = "unbound-params".into(),
            "selector-label" => changed.policy_mut().selector_node_id = "caller-chosen-label".into(),
            "selector-content" => {
                let id = changed.policy_mut().selector_node_id.clone();
                let node = changed.record.typed_semantics.foundation.provenance.nodes.iter_mut().find(|node| node.id == id).unwrap();
                let ValueProvenance::EntryWitness { field_path, .. } = &mut node.provenance else { panic!("selector root") };
                *field_path = "input_type.unauthenticated_tag".into();
                node.id = canonical_hash("cellscript-value-provenance-node-v1", &node.provenance).unwrap();
                let replacement = node.id.clone();
                changed.policy_mut().selector_node_id = replacement;
            }
            "unknown" => changed.policy_mut().unknown_selector = "accept".into(),
            "wrapper" => changed.record.typed_semantics.foundation.entry_contract.exact_entry = "action:mint".into(),
            _ => unreachable!(),
        }
        changed.rebind_policy_identity();
        let error = changed.check().unwrap_err();
        assert_eq!(error.code, CheckerRejectionCode::V2419TypedSemanticsInvalid, "{mutation}: {error}");
    }
}
