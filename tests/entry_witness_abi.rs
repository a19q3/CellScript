use cellscript_ckb_adapter::{place_entry_witness_payload_before_signing, EntryWitnessPlacementAbi};
use ckb_sdk::{
    constants::MultisigScript,
    traits::SecpCkbRawKeySigner,
    types::ScriptGroup,
    unlock::{MultisigConfig, ScriptSignError, ScriptSigner, SecpMultisigScriptSigner},
    SECP256K1,
};
use ckb_testtool::{
    ckb_hash::blake2b_256,
    ckb_types::{
        bytes::Bytes,
        core::{DepType, TransactionBuilder},
        packed,
        prelude::{Builder, Entity, Pack},
        H160,
    },
    context::Context,
};
use secp256k1::{PublicKey, SecretKey};

#[path = "support/ckb_script_runner.rs"]
#[allow(dead_code)]
mod ckb_script_runner;

use ckb_script_runner::{build_simple_fixture, compile_cellscript_source_to_elf, execute_cellscript_script};

const PARAMETERIZED_ENTRY: &str = r#"
module entry_witness_abi

action verify(witness expected: u64) -> u64 {
    verification
        require expected == 42
        return 0
}
"#;

const ALWAYS_SUCCESS_LOCK: &str = r#"
module entry_witness_always_success

action always_success() -> u64 {
    verification
        return 0
}
"#;

const U128_DYNAMIC_SCHEMA_ENTRY: &str = r#"
module entry_witness_u128_dynamic_schema

struct Entry {
    amount: u128,
    note: Vec<u8>,
}

action verify(witness left: Entry, witness right: Entry, witness expected_add: u128) -> u64 {
    verification
        require left.amount + right.amount == expected_add
        return 0
}
"#;

const U128_U64_ADD_ENTRY: &str = r#"
module entry_witness_u128_u64_add

action verify_add(witness wide: u128, witness delta: u64, witness expected: u128) -> u64 {
    verification
        require wide + delta == expected
        return 0
}
"#;

const U128_U64_SUB_ENTRY: &str = r#"
module entry_witness_u128_u64_sub

action verify_sub(witness wide: u128, witness delta: u64, witness expected: u128) -> u64 {
    verification
        require wide - delta == expected
        return 0
}
"#;

const SIGNED_TX_MAX_CYCLES: u64 = 70_000_000;

fn canonical_multisig_v2_witness(entry_payload: Bytes) -> packed::WitnessArgs {
    let signer_a = H160::from_slice(&[0x11; 20]).expect("20-byte signer hash");
    let signer_b = H160::from_slice(&[0x22; 20]).expect("20-byte signer hash");
    let config =
        MultisigConfig::new_with(MultisigScript::V2, vec![signer_a, signer_b], 0, 2).expect("canonical 2-of-2 multisig-v2 config");

    place_entry_witness_payload_before_signing(
        &config.placeholder_witness(),
        EntryWitnessPlacementAbi::WitnessArgsInputTypeV2,
        entry_payload,
    )
    .expect("place CellScript payload before signing")
}

fn signer_id(secret_key: &SecretKey) -> H160 {
    let public_key = PublicKey::from_secret_key(&SECP256K1, secret_key);
    H160::from_slice(&blake2b_256(public_key.serialize())[..20]).expect("20-byte signer hash")
}

fn raw_entry_payload(value: u64) -> Bytes {
    let mut payload = b"CSARGv1\0".to_vec();
    payload.extend_from_slice(&value.to_le_bytes());
    Bytes::from(payload)
}

/// Encode a dynamic-layout `Entry` as the Molecule table shape the runtime
/// decodes: `<u32 total><u32 offset amount><u32 offset note><amount><note>`.
/// The dynamic `Vec<u8>` field forces table decoding for the `u128` field.
fn molecule_dynamic_entry_bytes(amount: u128, note: &[u8], total_override: Option<u32>) -> Vec<u8> {
    let amount_offset = 4 + 4 * 2;
    let note_offset = amount_offset + 16;
    let total = note_offset + note.len();
    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(&total_override.unwrap_or(total as u32).to_le_bytes());
    bytes.extend_from_slice(&(amount_offset as u32).to_le_bytes());
    bytes.extend_from_slice(&(note_offset as u32).to_le_bytes());
    bytes.extend_from_slice(&amount.to_le_bytes());
    bytes.extend_from_slice(note);
    bytes
}

fn execute_u128_dynamic_schema_add(
    elf: &[u8],
    left: u128,
    right: u128,
    expected_add: u128,
    total_override: Option<u32>,
) -> ckb_script_runner::CkbScriptExecutionResult {
    let mut payload = b"CSARGv1\0".to_vec();
    for value in [left, right] {
        let entry = molecule_dynamic_entry_bytes(value, b"audit", total_override);
        payload.extend_from_slice(&(entry.len() as u32).to_le_bytes());
        payload.extend_from_slice(&entry);
    }
    payload.extend_from_slice(&expected_add.to_le_bytes());
    let witness = canonical_multisig_v2_witness(Bytes::from(payload));
    let mut fixture = build_simple_fixture(Bytes::default(), 1, 1);
    fixture.witnesses = vec![witness.as_bytes()];
    execute_cellscript_script(elf, &fixture)
}

fn execute_u128_u64_arithmetic(elf: &[u8], wide: u128, delta: u64, expected: u128) -> ckb_script_runner::CkbScriptExecutionResult {
    let mut payload = b"CSARGv1\0".to_vec();
    payload.extend_from_slice(&wide.to_le_bytes());
    payload.extend_from_slice(&delta.to_le_bytes());
    payload.extend_from_slice(&expected.to_le_bytes());
    let witness = canonical_multisig_v2_witness(Bytes::from(payload));
    let mut fixture = build_simple_fixture(Bytes::default(), 1, 1);
    fixture.witnesses = vec![witness.as_bytes()];
    execute_cellscript_script(elf, &fixture)
}

fn execute_on_second_group_input(witness: Bytes) -> ckb_script_runner::CkbScriptExecutionResult {
    let elf = compile_cellscript_source_to_elf(PARAMETERIZED_ENTRY, "verify", None);
    let mut fixture = build_simple_fixture(Bytes::default(), 2, 1);
    fixture.current_type_script_input_indices = vec![1];
    fixture.witnesses = vec![Bytes::from_static(b"unrelated-global-input-zero"), witness];
    execute_cellscript_script(&elf, &fixture)
}

fn execute_on_output_only_group(witness: Bytes) -> ckb_script_runner::CkbScriptExecutionResult {
    let elf = compile_cellscript_source_to_elf(PARAMETERIZED_ENTRY, "verify", None);
    let mut fixture = build_simple_fixture(Bytes::default(), 1, 1);
    fixture.current_type_script_input_indices.clear();
    fixture.witnesses = vec![witness];
    execute_cellscript_script(&elf, &fixture)
}

#[test]
fn signed_multisig_v2_lock_and_cellscript_type_execute_in_ckb_vm() -> Result<(), ScriptSignError> {
    let key_a = SecretKey::from_slice(&[0x11; 32]).expect("valid signer A key");
    let key_b = SecretKey::from_slice(&[0x22; 32]).expect("valid signer B key");
    let config = MultisigConfig::new_with(MultisigScript::V2, vec![signer_id(&key_a), signer_id(&key_b)], 0, 2)?;

    let mut context = Context::new_with_deterministic_rng();
    let multisig_v2 = ckb_system_scripts_v0_6_0::BUNDLED_CELL
        .get("specs/cells/secp256k1_blake160_multisig_all")
        .expect("bundled multisig-v2 script");
    context.deploy_cell(Bytes::copy_from_slice(&multisig_v2));
    let secp256k1_data = ckb_system_scripts_v0_6_0::BUNDLED_CELL.get("specs/cells/secp256k1_data").expect("bundled secp256k1 data");
    let secp256k1_data_out_point = context.deploy_cell(Bytes::copy_from_slice(&secp256k1_data));

    let always_success_elf = compile_cellscript_source_to_elf(ALWAYS_SUCCESS_LOCK, "always_success", None);
    let always_success_out_point = context.deploy_cell(Bytes::from(always_success_elf));
    let always_success_lock = context.build_script(&always_success_out_point, Bytes::default()).expect("build always-success lock");

    let cellscript_elf = compile_cellscript_source_to_elf(PARAMETERIZED_ENTRY, "verify", None);
    let cellscript_out_point = context.deploy_cell(Bytes::from(cellscript_elf));
    let cellscript_type = context.build_script(&cellscript_out_point, Bytes::default()).expect("build CellScript type script");
    let multisig_lock: packed::Script = (&config).into();

    let unrelated_input = context.create_cell(
        packed::CellOutput::new_builder()
            .capacity::<packed::Uint64>(100_000_000_000u64.pack())
            .lock(always_success_lock.clone())
            .build(),
        Bytes::default(),
    );
    let multisig_input = context.create_cell(
        packed::CellOutput::new_builder()
            .capacity::<packed::Uint64>(100_000_000_000u64.pack())
            .lock(multisig_lock.clone())
            .type_(Some(cellscript_type.clone()).pack())
            .build(),
        Bytes::default(),
    );
    let output = packed::CellOutput::new_builder()
        .capacity::<packed::Uint64>(190_000_000_000u64.pack())
        .lock(always_success_lock)
        .type_(Some(cellscript_type).pack())
        .build();

    let unsigned_witness = place_entry_witness_payload_before_signing(
        &config.placeholder_witness(),
        EntryWitnessPlacementAbi::WitnessArgsInputTypeV2,
        raw_entry_payload(42),
    )
    .expect("place entry payload before signing");
    let tx = TransactionBuilder::default()
        .inputs([
            packed::CellInput::new_builder().previous_output(unrelated_input).build(),
            packed::CellInput::new_builder().previous_output(multisig_input).build(),
        ])
        .output(output)
        .output_data(Bytes::default().pack())
        .cell_dep(packed::CellDep::new_builder().out_point(secp256k1_data_out_point).dep_type(DepType::Code).build())
        .witnesses([Bytes::from_static(b"unrelated-global-input-zero"), unsigned_witness.as_bytes()].pack())
        .build();
    let tx = context.complete_tx(tx);

    let raw_signer = SecpCkbRawKeySigner::new_with_secret_keys(vec![key_a, key_b]);
    let signer = SecpMultisigScriptSigner::new(Box::new(raw_signer), config);
    let mut lock_group = ScriptGroup::from_lock_script(&multisig_lock);
    lock_group.input_indices.push(1);
    let signed_tx = signer.sign_tx(&tx, &lock_group)?;

    let signed_witness = packed::WitnessArgs::from_slice(signed_tx.witnesses().get(1).expect("multisig witness").raw_data().as_ref())
        .expect("signed WitnessArgs");
    let lock = signed_witness.lock().to_opt().expect("signed multisig lock").raw_data();
    let signature_offset = 4 + 2 * 20;
    assert_eq!(&lock[..4], &[0, 0, 2, 2], "canonical 2-of-2 multisig header");
    assert!(lock[signature_offset..].iter().any(|byte| *byte != 0), "multisig signatures must be populated");
    context.verify_tx(&signed_tx, SIGNED_TX_MAX_CYCLES).expect("multisig-v2 lock and CellScript type script must both pass");

    // A valid, otherwise unused output_type mutation keeps the CellScript
    // input_type payload valid, but must invalidate the multisig signature.
    let tampered_witness = signed_witness.as_builder().output_type(Some(Bytes::from_static(b"post-signing-mutation")).pack()).build();
    let mut witnesses: Vec<packed::Bytes> = signed_tx.witnesses().into_iter().collect();
    witnesses[1] = tampered_witness.as_bytes().pack();
    let tampered_tx = signed_tx.as_advanced_builder().set_witnesses(witnesses).build();
    assert!(
        context.verify_tx(&tampered_tx, SIGNED_TX_MAX_CYCLES).is_err(),
        "mutating WitnessArgs after signing must invalidate multisig-v2"
    );

    Ok(())
}

#[test]
fn raw_v1_group_input_payload_is_rejected_by_placement_abi_v2() {
    let result = execute_on_second_group_input(raw_entry_payload(42));
    assert_eq!(
        result.exit_code, 25,
        "placement ABI v2 must require WitnessArgs.input_type instead of accepting a raw payload alias: {:?}",
        result.captured_debug
    );
}

#[test]
fn witnessargs_input_type_falls_back_to_group_output_zero() {
    let witness = canonical_multisig_v2_witness(raw_entry_payload(42));
    let result = execute_on_output_only_group(witness.as_bytes());
    assert_eq!(result.exit_code, 0, "an output-only type group must resolve GroupOutput#0: {:?}", result.captured_debug);
}

#[test]
fn witnessargs_output_type_is_not_an_entry_payload_alias() {
    let witness = canonical_multisig_v2_witness(raw_entry_payload(42))
        .as_builder()
        .input_type(None::<Bytes>.pack())
        .output_type(Some(raw_entry_payload(42)).pack())
        .build();
    let result = execute_on_second_group_input(witness.as_bytes());
    assert_eq!(result.exit_code, 25, "wrong WitnessArgs field must fail closed: {:?}", result.captured_debug);
}

#[test]
fn malformed_witnessargs_input_type_length_fails_closed() {
    let witness = canonical_multisig_v2_witness(raw_entry_payload(42));
    let mut encoded = witness.as_slice().to_vec();
    let input_type_offset = u32::from_le_bytes(encoded[8..12].try_into().expect("input_type table offset")) as usize;
    let declared_len =
        u32::from_le_bytes(encoded[input_type_offset..input_type_offset + 4].try_into().expect("input_type Bytes length"));
    encoded[input_type_offset..input_type_offset + 4].copy_from_slice(&(declared_len + 1).to_le_bytes());

    let result = execute_on_second_group_input(Bytes::from(encoded));
    assert_eq!(result.exit_code, 25, "malformed Molecule must fail closed: {:?}", result.captured_debug);
}

#[test]
fn malformed_unselected_witnessargs_field_still_fails_closed() {
    let witness = canonical_multisig_v2_witness(raw_entry_payload(42))
        .as_builder()
        .output_type(Some(Bytes::from_static(b"protocol-output-data")).pack())
        .build();
    let mut encoded = witness.as_slice().to_vec();
    let output_type_offset = u32::from_le_bytes(encoded[12..16].try_into().expect("output_type table offset")) as usize;
    let declared_len =
        u32::from_le_bytes(encoded[output_type_offset..output_type_offset + 4].try_into().expect("output_type Bytes length"));
    encoded[output_type_offset..output_type_offset + 4].copy_from_slice(&(declared_len + 1).to_le_bytes());

    let result = execute_on_second_group_input(Bytes::from(encoded));
    assert_eq!(result.exit_code, 25, "the placement ABI must validate the whole WitnessArgs table: {:?}", result.captured_debug);
}

#[test]
fn u128_add_on_dynamic_schema_fields_executes_exactly_in_ckb_vm() {
    let elf = compile_cellscript_source_to_elf(U128_DYNAMIC_SCHEMA_ENTRY, "verify", None);
    let vectors: [(u128, u128); 3] = [
        (0x00ff_ffee_ddcc_bbaa_55aa_55aa_55aa_55aa, 0x55aa_55aa_55aa_55aa_aa55_aa55_aa55_aa55),
        (0x0123_4567_89ab_cdef_fedc_ba98_7654_3210, 0xf0f0_0f0f_f0f0_0f0f_aaaa_5555_aaaa_5555),
        (1, u128::MAX - 1),
    ];
    for (left, right) in vectors {
        let expected_add = left.checked_add(right).expect("vector must not overflow");
        let result = execute_u128_dynamic_schema_add(&elf, left, right, expected_add, None);
        assert_eq!(
            result.exit_code, 0,
            "u128 addition over Molecule-table-decoded fields must execute exactly in CKB-VM: {:?}",
            result.captured_debug
        );
    }

    let wrong = execute_u128_dynamic_schema_add(&elf, 1, 2, 4, None);
    assert_eq!(wrong.exit_code, 5, "a wrong expected sum must fail the assertion: {:?}", wrong.captured_debug);

    let malformed = execute_u128_dynamic_schema_add(&elf, 8, 1, 9, Some(255));
    assert_eq!(
        malformed.exit_code, 2,
        "a mismatched Molecule table length must fail the bounds check: {:?}",
        malformed.captured_debug
    );
}

#[test]
fn u128_u64_arithmetic_checks_carry_overflow_and_underflow_in_ckb_vm() {
    let add_elf = compile_cellscript_source_to_elf(U128_U64_ADD_ENTRY, "verify_add", None);
    let carried = execute_u128_u64_arithmetic(&add_elf, u64::MAX as u128, 1, 1u128 << 64);
    assert_eq!(carried.exit_code, 0, "u128 + u64 carry must execute exactly: {:?}", carried.captured_debug);

    let overflow = execute_u128_u64_arithmetic(&add_elf, u128::MAX, 1, 0);
    assert_eq!(overflow.exit_code, 49, "u128 + u64 overflow must fail closed: {:?}", overflow.captured_debug);

    let sub_elf = compile_cellscript_source_to_elf(U128_U64_SUB_ENTRY, "verify_sub", None);
    let borrowed = execute_u128_u64_arithmetic(&sub_elf, 1u128 << 64, 1, u64::MAX as u128);
    assert_eq!(borrowed.exit_code, 0, "u128 - u64 borrow must execute exactly: {:?}", borrowed.captured_debug);

    let underflow = execute_u128_u64_arithmetic(&sub_elf, 0, 1, 0);
    assert_eq!(underflow.exit_code, 49, "u128 - u64 underflow must fail closed: {:?}", underflow.captured_debug);
}
