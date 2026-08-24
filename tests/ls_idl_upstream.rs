use std::collections::BTreeSet;

use base64::Engine as _;
use cellscript::package::registry::validate_ls_idl_document;
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};

const CLIENT_VECTORS_SHA256: &str = "a9a6dca4fd0c5fcd2ca7aea6468784be7fdb29d6274049f07090cbab0ce9c1bb";

fn decode_fixture(encoded: &str) -> Vec<u8> {
    let compact: String = encoded.chars().filter(|character| !character.is_whitespace()).collect();
    base64::engine::general_purpose::STANDARD.decode(compact).expect("valid fixture Base64")
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

#[test]
fn registry_admits_every_pinned_upstream_idl_document_without_reserializing() {
    let fixtures: Vec<(&str, Vec<u8>, &str)> = vec![
        (
            "derive/multisig-2of2-nonce",
            include_bytes!("compat/ls_idl/derive/multisig-2of2-nonce.json").to_vec(),
            "587098bbe12e37a7394d06ff711a59242f033759e9ba7f5b62b8f6a234275063",
        ),
        (
            "derive/pow-lock",
            include_bytes!("compat/ls_idl/derive/pow-lock.json").to_vec(),
            "d551803734459f28b2849f13b2111778d3753b518701a86a434e9438df86e2d6",
        ),
        (
            "derive/schnorr-pubkey-recovery",
            include_bytes!("compat/ls_idl/derive/schnorr-pubkey-recovery.json").to_vec(),
            "b37329b5fb13b25de94ef068724839f356096bc3516dda461b516ee983a8d371",
        ),
        (
            "derive/secp256k1-timelock",
            include_bytes!("compat/ls_idl/derive/secp256k1-timelock.json").to_vec(),
            "056bc4f2b11bc7f0dfead9f2dcc0ec5097b42b353d4577b3836ef872b121710f",
        ),
        (
            "derive/simple-lock",
            include_bytes!("compat/ls_idl/derive/simple-lock.json").to_vec(),
            "d28abead992546908eb483c24667e58302f193c00e08f6cbed1a6302995ca1c0",
        ),
        (
            "scripts/simple-lock",
            decode_fixture(include_str!("compat/ls_idl/scripts/simple-lock.idl.json.b64")),
            "6fd2ab0171167c6862582c4e95a6de7b1cd153f77a936af7e52be6599ddddd31",
        ),
        (
            "scripts/timelock-lock",
            decode_fixture(include_str!("compat/ls_idl/scripts/timelock-lock.idl.json.b64")),
            "18ae57828b5fbd0c8df0900eed1153e7585587d4049900c50729616227a9beda",
        ),
    ];

    for (name, bytes, expected_sha256) in fixtures {
        assert_eq!(sha256_hex(&bytes), expected_sha256, "raw-byte drift in {name}");
        validate_ls_idl_document(&bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
    }
}

#[test]
fn registry_schema_tracks_the_complete_pinned_upstream_client_vector_corpus() {
    let bytes = decode_fixture(include_str!("compat/ls_idl/ckb-idl-client-test-vectors.json.b64"));
    assert_eq!(sha256_hex(&bytes), CLIENT_VECTORS_SHA256);

    let document: Value = serde_json::from_slice(&bytes).expect("valid upstream vector JSON");
    let vectors = document["vectors"].as_array().expect("upstream vectors array");
    assert_eq!(vectors.len(), 17, "review the compatibility profile when upstream adds vectors");

    let mut observed_types = BTreeSet::new();
    let mut rejected_ids = Vec::new();
    for vector in vectors {
        let id = vector["id"].as_str().expect("vector id");
        let fields = vector["fields"].as_array().expect("vector fields");
        for field in fields {
            observed_types.insert(field["type"].as_str().expect("field type").to_string());
        }
        let registry_document = serde_json::to_vec(&json!({ "witness": fields })).expect("serialize Registry schema probe");
        match validate_ls_idl_document(&registry_document) {
            Ok(()) => assert_ne!(id, "unknown-type-rejected", "unknown types must fail closed"),
            Err(error) => {
                assert_eq!(id, "unknown-type-rejected", "unexpected rejection for {id}: {error}");
                assert!(error.contains("must be one of"));
                rejected_ids.push(id);
            }
        }
    }

    assert_eq!(rejected_ids, ["unknown-type-rejected"]);
    assert_eq!(
        observed_types,
        BTreeSet::from([
            "bytes".to_string(),
            "molecule_bytes".to_string(),
            "schnorr_sig".to_string(),
            "secp256k1_sig".to_string(),
            "uint32".to_string(),
            "uint64".to_string(),
            "uint8".to_string(),
        ])
    );
}
