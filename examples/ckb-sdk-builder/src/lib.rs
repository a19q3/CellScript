//! Cookbook wrapper for the formal CellScript CKB adapter crate.
//!
//! The reusable implementation lives in `crates/cellscript-ckb-adapter`.
//! This example crate stays intentionally small so the checked-in cookbook does
//! not become a second adapter implementation.

pub use cellscript_ckb_adapter::*;

#[cfg(test)]
mod tests {
    use super::*;
    use ckb_types::{bytes::Bytes, packed::WitnessArgs, prelude::*};

    #[test]
    fn cookbook_uses_formal_adapter_crate() {
        let resolved = sample_resolved_action_tx();
        let (_tx, evidence) = build_action_transaction(&resolved).unwrap();

        assert_eq!(evidence.schema, ACTION_ACCEPTANCE_REPORT_SCHEMA);
        assert_eq!(evidence.state, "ResolvedActionTx");
        assert!(!evidence.ckb_vm_execution);
        assert!(!evidence.tx_pool_acceptance);
    }

    #[test]
    fn cookbook_places_entry_payload_in_witnessargs_input_type_before_signing() {
        let base = WitnessArgs::new_builder()
            .lock(Some(Bytes::from(vec![0u8; 65])).pack())
            .output_type(Some(Bytes::from_static(b"preserved-output-type")).pack())
            .build();
        let payload = Bytes::from_static(b"CSARGv1\0\x2a\0\0\0\0\0\0\0");

        let witness =
            place_entry_witness_payload_before_signing(&base, EntryWitnessPlacementAbi::WitnessArgsInputTypeV2, payload.clone())
                .expect("canonical entry placement should succeed before signing");

        assert_eq!(witness.lock().to_opt().expect("lock placeholder preserved").raw_data().len(), 65);
        assert_eq!(witness.input_type().to_opt().expect("entry payload placed").raw_data(), payload);
        assert_eq!(
            witness.output_type().to_opt().expect("output_type preserved").raw_data(),
            Bytes::from_static(b"preserved-output-type")
        );
        assert_eq!(EntryWitnessPlacementAbi::WitnessArgsInputTypeV2.name(), ENTRY_WITNESS_PLACEMENT_ABI);
    }
}
