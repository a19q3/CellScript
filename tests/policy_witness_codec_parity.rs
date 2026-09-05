//! Cross-package wire equality without a production compiler dependency in the
//! adapter or out-of-package source includes in either publishable crate.

use cellscript::policy_witness as compiler;
use cellscript_ckb_adapter::policy_witness as adapter;

const GOLDEN_HEX: &str = concat!(
    "4353504f4c763100",
    "8f0000000c00000052000000",
    "4600000014000000150000003500000039000000",
    "00",
    "0000000000000000000000000000000000000000000000000000000000000000",
    "07000000090000004353415247763100aa",
    "3d00000014000000150000003500000039000000",
    "01",
    "1111111111111111111111111111111111111111111111111111111111111111",
    "0403020100000000",
);

#[test]
fn independent_compiler_and_adapter_codecs_match_literal_bytes_and_limits() {
    let golden = hex::decode(GOLDEN_HEX).unwrap();
    let compiler_records = compiler::decode_policy_witness_bundle(&golden).unwrap();
    let adapter_records = adapter::decode_policy_witness_bundle(&golden).unwrap();
    for (left, right) in compiler_records.iter().zip(&adapter_records) {
        assert_eq!(left.role as u8, right.role as u8);
        assert_eq!(left.script_hash, right.script_hash);
        assert_eq!(left.tag, right.tag);
        assert_eq!(left.args, right.args);
    }
    assert_eq!(compiler_records.len(), 2);
    assert_eq!(adapter_records.len(), 2);
    assert_eq!(compiler::encode_policy_witness_bundle(&compiler_records).unwrap(), golden);
    assert_eq!(adapter::encode_policy_witness_bundle(&adapter_records).unwrap(), golden);
    assert_eq!(compiler::POLICY_WITNESS_ABI, adapter::POLICY_WITNESS_ABI);
    assert_eq!(compiler::POLICY_WITNESS_MAGIC, adapter::POLICY_WITNESS_MAGIC);
    assert_eq!(compiler::MAX_POLICY_WITNESS_BYTES, adapter::MAX_POLICY_WITNESS_BYTES);
    assert_eq!(compiler::MAX_POLICY_WITNESS_BUNDLE_BYTES, adapter::MAX_POLICY_WITNESS_BUNDLE_BYTES);
    assert_eq!(compiler::MAX_POLICY_WITNESS_RECORDS, adapter::MAX_POLICY_WITNESS_RECORDS);
    assert_eq!(compiler::MAX_POLICY_ARTIFACT_VARIANTS, adapter::MAX_POLICY_ARTIFACT_VARIANTS);
}

#[test]
fn independent_codecs_reject_identical_malformed_literal_variants() {
    let golden = hex::decode(GOLDEN_HEX).unwrap();
    for length in 0..golden.len() {
        assert!(compiler::decode_policy_witness_bundle(&golden[..length]).is_err());
        assert!(adapter::decode_policy_witness_bundle(&golden[..length]).is_err());
    }
    for offset in [0, 6, 8, 12, 16, 20, 24, 28, 32, 36, 40, 77, 81, 90, 94, 98, 102, 106, 110, 147] {
        let mut malformed = golden.clone();
        malformed[offset] = 0xff;
        assert!(compiler::decode_policy_witness_bundle(&malformed).is_err(), "compiler accepted {offset}");
        assert!(adapter::decode_policy_witness_bundle(&malformed).is_err(), "adapter accepted {offset}");
    }
}
