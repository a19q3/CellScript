//! Bounded host codec for the versioned multi-policy witness envelope.
//!
//! The codec is kept independent of the compiler; placement uses CKB packed
//! types. Structural canonicality does not validate a selected action's ABI.

use std::fmt;

pub const POLICY_WITNESS_ABI: &str = "cellscript-policy-witness-v1";
pub const POLICY_WITNESS_MAGIC: &[u8; 8] = b"CSPOLv1\0";
pub const MAX_POLICY_WITNESS_BYTES: usize = 4096;
/// An otherwise empty WitnessArgs costs 16 table bytes plus 4 Bytes bytes.
pub const MAX_POLICY_WITNESS_BUNDLE_BYTES: usize = MAX_POLICY_WITNESS_BYTES - 20;
pub const MAX_POLICY_WITNESS_RECORDS: usize = 8;
/// The artifact resolver, not this record codec, enforces the variant bound.
pub const MAX_POLICY_ARTIFACT_VARIANTS: usize = 64;

const ENTRY_ARGS_MAGIC: &[u8; 8] = b"CSARGv1\0";
const RECORD_HEADER_BYTES: usize = 20;
const RECORD_FIXED_BYTES: usize = 61;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum PolicyScriptRole {
    Lock = 0,
    Type = 1,
}

impl PolicyScriptRole {
    pub const fn as_byte(self) -> u8 {
        self as u8
    }

    fn from_byte(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Lock),
            1 => Ok(Self::Type),
            _ => Err(PolicyWitnessError::UnknownRole(value)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyWitnessRecord {
    pub role: PolicyScriptRole,
    pub script_hash: [u8; 32],
    pub tag: u32,
    pub args: Vec<u8>,
}

impl PolicyWitnessRecord {
    fn key(&self) -> (PolicyScriptRole, [u8; 32]) {
        (self.role, self.script_hash)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyWitnessError {
    RecordCount(usize),
    SizeLimit,
    InvalidMagic,
    InvalidStructure(&'static str),
    UnknownRole(u8),
    DuplicateKey,
    NonCanonicalOrder,
    InvalidArgsMagic,
}

impl fmt::Display for PolicyWitnessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RecordCount(count) => {
                write!(formatter, "policy witness needs 1..={MAX_POLICY_WITNESS_RECORDS} records; got {count}")
            }
            Self::SizeLimit => write!(formatter, "policy witness bundle exceeds {MAX_POLICY_WITNESS_BUNDLE_BYTES} bytes"),
            Self::InvalidMagic => formatter.write_str("policy witness must start with CSPOLv1\\0"),
            Self::InvalidStructure(detail) => write!(formatter, "non-canonical policy witness: {detail}"),
            Self::UnknownRole(role) => write!(formatter, "unknown policy witness Script role {role}; expected 0 (Lock) or 1 (Type)"),
            Self::DuplicateKey => formatter.write_str("duplicate policy witness role and full Script hash"),
            Self::NonCanonicalOrder => {
                formatter.write_str("policy witness records must be strictly ordered by role and full Script hash")
            }
            Self::InvalidArgsMagic => formatter.write_str("nonempty policy args must start with CSARGv1\\0"),
        }
    }
}

impl std::error::Error for PolicyWitnessError {}

pub type Result<T> = std::result::Result<T, PolicyWitnessError>;

/// Encode a canonical bundle, sorting records without changing the caller's list.
///
/// There must be exactly one record per (role, full Script hash), even if tags
/// differ. Empty args are preserved. Nonempty args are only magic-checked here;
/// their exact layout and the tag must be checked against the selected artifact.
pub fn encode_policy_witness_bundle(records: &[PolicyWitnessRecord]) -> Result<Vec<u8>> {
    validate_record_count(records.len())?;
    let mut ordered = records.iter().collect::<Vec<_>>();
    ordered.sort_unstable_by_key(|record| record.key());
    if ordered.windows(2).any(|pair| pair[0].key() == pair[1].key()) {
        return Err(PolicyWitnessError::DuplicateKey);
    }

    let header_bytes = 4 * (records.len() + 1);
    let mut total_bytes = POLICY_WITNESS_MAGIC.len() + header_bytes;
    for record in &ordered {
        validate_args(&record.args)?;
        total_bytes = total_bytes
            .checked_add(RECORD_FIXED_BYTES)
            .and_then(|total| total.checked_add(record.args.len()))
            .filter(|total| *total <= MAX_POLICY_WITNESS_BUNDLE_BYTES)
            .ok_or(PolicyWitnessError::SizeLimit)?;
    }

    let mut encoded = Vec::with_capacity(total_bytes);
    encoded.extend_from_slice(POLICY_WITNESS_MAGIC);
    write_u32(&mut encoded, total_bytes - POLICY_WITNESS_MAGIC.len());
    let mut offset = header_bytes;
    for record in &ordered {
        write_u32(&mut encoded, offset);
        offset += RECORD_FIXED_BYTES + record.args.len();
    }
    for record in ordered {
        write_u32(&mut encoded, RECORD_FIXED_BYTES + record.args.len());
        for field_offset in [RECORD_HEADER_BYTES, 21, 53, 57] {
            write_u32(&mut encoded, field_offset);
        }
        encoded.push(record.role.as_byte());
        encoded.extend_from_slice(&record.script_hash);
        encoded.extend_from_slice(&record.tag.to_le_bytes());
        write_u32(&mut encoded, record.args.len());
        encoded.extend_from_slice(&record.args);
    }
    Ok(encoded)
}

/// Decode only the canonical v1 layout. No offsets or lengths are trusted before
/// byte/count checks, and no trailing bytes or compatible extra fields are allowed.
pub fn decode_policy_witness_bundle(encoded: &[u8]) -> Result<Vec<PolicyWitnessRecord>> {
    if encoded.len() > MAX_POLICY_WITNESS_BUNDLE_BYTES {
        return Err(PolicyWitnessError::SizeLimit);
    }
    let vector = encoded.strip_prefix(POLICY_WITNESS_MAGIC).ok_or(PolicyWitnessError::InvalidMagic)?;
    if read_u32(vector, 0)? != vector.len() {
        return Err(PolicyWitnessError::InvalidStructure("DynVec total size does not match the bundle"));
    }
    if vector.len() == 4 {
        return Err(PolicyWitnessError::RecordCount(0));
    }
    let first_offset = read_u32(vector, 4)?;
    if first_offset < 8 || first_offset % 4 != 0 || first_offset > vector.len() {
        return Err(PolicyWitnessError::InvalidStructure("invalid DynVec header"));
    }
    let count = first_offset / 4 - 1;
    validate_record_count(count)?;
    let mut records = Vec::<PolicyWitnessRecord>::with_capacity(count);
    for index in 0..count {
        let start = read_u32(vector, 4 * (index + 1))?;
        let end = if index + 1 == count { vector.len() } else { read_u32(vector, 4 * (index + 2))? };
        if start < first_offset || start >= end || end > vector.len() {
            return Err(PolicyWitnessError::InvalidStructure("invalid or overlapping record offsets"));
        }
        let record = decode_record(&vector[start..end])?;
        if let Some(previous) = records.last() {
            match previous.key().cmp(&record.key()) {
                std::cmp::Ordering::Equal => return Err(PolicyWitnessError::DuplicateKey),
                std::cmp::Ordering::Greater => return Err(PolicyWitnessError::NonCanonicalOrder),
                std::cmp::Ordering::Less => {}
            }
        }
        records.push(record);
    }
    Ok(records)
}

/// Select from a successfully decoded (or equivalently validated) record list.
///
/// An absent key is not a fallback selector. The executing artifact must reject
/// absence and validate the returned tag and exact variant argument schema.
pub fn selected_record<'a>(
    records: &'a [PolicyWitnessRecord],
    role: PolicyScriptRole,
    script_hash: &[u8; 32],
) -> Option<&'a PolicyWitnessRecord> {
    records.iter().find(|record| record.role == role && record.script_hash == *script_hash)
}

fn validate_record_count(count: usize) -> Result<()> {
    if !(1..=MAX_POLICY_WITNESS_RECORDS).contains(&count) {
        return Err(PolicyWitnessError::RecordCount(count));
    }
    Ok(())
}

fn validate_args(args: &[u8]) -> Result<()> {
    if !args.is_empty() && !args.starts_with(ENTRY_ARGS_MAGIC) {
        return Err(PolicyWitnessError::InvalidArgsMagic);
    }
    Ok(())
}

fn decode_record(encoded: &[u8]) -> Result<PolicyWitnessRecord> {
    if encoded.len() < RECORD_FIXED_BYTES || read_u32(encoded, 0)? != encoded.len() {
        return Err(PolicyWitnessError::InvalidStructure("record total size does not match its range"));
    }
    for (index, expected) in [RECORD_HEADER_BYTES, 21, 53, 57].into_iter().enumerate() {
        if read_u32(encoded, 4 * (index + 1))? != expected {
            return Err(PolicyWitnessError::InvalidStructure("record must have exactly the four fixed v1 field offsets"));
        }
    }
    let role = PolicyScriptRole::from_byte(encoded[20])?;
    let script_hash = encoded[21..53].try_into().expect("fixed hash range is checked by the record minimum size");
    let tag = u32::from_le_bytes(encoded[53..57].try_into().expect("fixed tag range is checked by the record minimum size"));
    if read_u32(encoded, 57)? != encoded.len() - RECORD_FIXED_BYTES {
        return Err(PolicyWitnessError::InvalidStructure("args Bytes length does not match its range"));
    }
    let args = &encoded[RECORD_FIXED_BYTES..];
    validate_args(args)?;
    Ok(PolicyWitnessRecord { role, script_hash, tag, args: args.to_vec() })
}

fn read_u32(encoded: &[u8], offset: usize) -> Result<usize> {
    let end = offset.checked_add(4).ok_or(PolicyWitnessError::InvalidStructure("integer offset overflow"))?;
    let bytes = encoded.get(offset..end).ok_or(PolicyWitnessError::InvalidStructure("truncated u32"))?;
    Ok(u32::from_le_bytes(bytes.try_into().expect("u32 range has exactly four bytes")) as usize)
}

fn write_u32(encoded: &mut Vec<u8>, value: usize) {
    // All encoded values are bounded by MAX_POLICY_WITNESS_BUNDLE_BYTES.
    encoded.extend_from_slice(&(value as u32).to_le_bytes());
}

/// Place a policy bundle in an unsigned WitnessArgs draft.
///
/// Rejects an occupied input_type, including an existing policy bundle. The
/// caller must aggregate all requests before placement and sign afterward;
/// arbitrary lock bytes cannot reveal whether signing has already occurred.
pub fn place_policy_witness_bundle_before_signing(
    base: &ckb_types::packed::WitnessArgs,
    bundle: &[u8],
) -> anyhow::Result<ckb_types::packed::WitnessArgs> {
    use ckb_types::{bytes::Bytes, packed::WitnessArgs, prelude::*};

    decode_policy_witness_bundle(bundle)?;
    let base = WitnessArgs::from_slice(base.as_slice()).map_err(|error| anyhow::anyhow!("invalid base WitnessArgs: {error}"))?;
    if base.input_type().to_opt().is_some() {
        anyhow::bail!("refusing to overwrite WitnessArgs.input_type");
    }
    let witness = base.as_builder().input_type(Some(Bytes::copy_from_slice(bundle)).pack()).build();
    if witness.as_slice().len() > MAX_POLICY_WITNESS_BYTES {
        anyhow::bail!("serialized policy WitnessArgs exceeds {MAX_POLICY_WITNESS_BYTES} bytes");
    }
    Ok(witness)
}

// Test-only source inclusion avoids a compiler dependency in the adapter.
// Production copies intentionally remain separate and share literal vectors.
#[cfg(test)]
mod tests {
    use super::*;
    use ckb_types::{bytes::Bytes, packed::WitnessArgs, prelude::*};

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

    fn golden_bytes() -> Vec<u8> {
        hex::decode(GOLDEN_HEX).unwrap()
    }

    #[test]
    fn adapter_matches_the_independent_literal_vector() {
        let golden = golden_bytes();
        let expected = vec![
            PolicyWitnessRecord { role: PolicyScriptRole::Lock, script_hash: [0; 32], tag: 7, args: b"CSARGv1\0\xaa".to_vec() },
            PolicyWitnessRecord { role: PolicyScriptRole::Type, script_hash: [0x11; 32], tag: 0x01020304, args: Vec::new() },
        ];
        assert_eq!(decode_policy_witness_bundle(&golden).unwrap(), expected);
        assert_eq!(encode_policy_witness_bundle(&expected).unwrap(), golden);
    }

    #[test]
    fn adapter_rejects_malformed_literal_variants() {
        let golden = golden_bytes();
        for length in 0..golden.len() {
            assert!(decode_policy_witness_bundle(&golden[..length]).is_err());
        }
        for offset in [0, 6, 8, 12, 16, 20, 24, 28, 32, 36, 40, 77, 81, 90, 94, 98, 102, 106, 110, 147] {
            let mut malformed = golden.clone();
            malformed[offset] = 0xff;
            assert!(decode_policy_witness_bundle(&malformed).is_err(), "adapter accepted mutation {offset}");
        }
    }

    #[test]
    fn placement_preserves_lock_and_output_type_and_refuses_occupied_input_type() {
        let lock = Bytes::from(vec![0x42; 65]);
        let output_type = Bytes::from_static(b"other protocol");
        let base = WitnessArgs::new_builder().lock(Some(lock.clone()).pack()).output_type(Some(output_type.clone()).pack()).build();
        let bundle = golden_bytes();
        let witness = place_policy_witness_bundle_before_signing(&base, &bundle).unwrap();
        assert_eq!(witness.lock().to_opt().unwrap().raw_data(), lock);
        assert_eq!(witness.output_type().to_opt().unwrap().raw_data(), output_type);
        assert_eq!(witness.input_type().to_opt().unwrap().raw_data().as_ref(), bundle.as_slice());
        for occupied in [
            witness,
            base.clone().as_builder().input_type(Some(Bytes::new()).pack()).build(),
            base.clone().as_builder().input_type(Some(Bytes::from_static(b"CSARGv1\0")).pack()).build(),
        ] {
            let error = place_policy_witness_bundle_before_signing(&occupied, &bundle).unwrap_err();
            assert!(error.to_string().contains("refusing to overwrite"), "{error}");
        }
        assert!(place_policy_witness_bundle_before_signing(&base, b"CSARGv1\0").is_err());
    }

    #[test]
    fn placement_counts_signature_placeholders_and_other_fields_in_the_final_limit() {
        let max_args = MAX_POLICY_WITNESS_BUNDLE_BYTES - 8 - 8 - 61;
        let mut args = b"CSARGv1\0".to_vec();
        args.resize(max_args, 0);
        let bundle =
            encode_policy_witness_bundle(&[PolicyWitnessRecord { role: PolicyScriptRole::Type, script_hash: [7; 32], tag: 0, args }])
                .unwrap();
        let empty = WitnessArgs::new_builder().build();
        let full = place_policy_witness_bundle_before_signing(&empty, &bundle).unwrap();
        assert_eq!(full.as_slice().len(), MAX_POLICY_WITNESS_BYTES);
        for base in [
            empty.clone().as_builder().lock(Some(Bytes::from(vec![0; 65])).pack()).build(),
            empty.as_builder().output_type(Some(Bytes::from_static(b"x")).pack()).build(),
        ] {
            let error = place_policy_witness_bundle_before_signing(&base, &bundle).unwrap_err();
            assert!(error.to_string().contains("exceeds 4096"), "{error}");
        }
        let invalid_base = WitnessArgs::new_unchecked(Bytes::from_static(b"bad"));
        assert!(place_policy_witness_bundle_before_signing(&invalid_base, &golden_bytes()).is_err());
    }
}
