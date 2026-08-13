//! Illustrative `ckb-idl-derive` input for `idl.json`.
//! This is not a complete or audited CKB Lock Script.

use ckb_idl_derive::CkbWitness;

#[derive(CkbWitness)]
struct DemoLockWitness {
    #[witness(description = "Recoverable CKB secp256k1 signature")]
    signature: [u8; 65],
    nonce: u64,
    #[witness(
        required = false,
        description = "Length-prefixed application bytes; required=false is descriptive in 0.1"
    )]
    memo: Vec<u8>,
}
