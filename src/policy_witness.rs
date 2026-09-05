//! Bounded host codec for the versioned multi-policy witness envelope.
//!
//! This module has no compiler or CKB dependency. Decoding proves structural
//! canonicality only; the selected artifact must validate its tag and args ABI.

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

#[cfg(test)]
mod tests {
    use super::*;

    // Independent literal Molecule vector: Lock(00..00, tag 7, CSARGv1 + aa),
    // then Type(11..11, tag 0x01020304, empty args). DynVec size 143.
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
        GOLDEN_HEX.as_bytes().chunks_exact(2).map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap()).collect()
    }

    fn golden_records() -> Vec<PolicyWitnessRecord> {
        vec![
            PolicyWitnessRecord { role: PolicyScriptRole::Lock, script_hash: [0; 32], tag: 7, args: b"CSARGv1\0\xaa".to_vec() },
            PolicyWitnessRecord { role: PolicyScriptRole::Type, script_hash: [0x11; 32], tag: 0x01020304, args: Vec::new() },
        ]
    }

    fn replace_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    #[test]
    fn literal_golden_vector_pins_both_roles_offsets_and_empty_args() {
        let expected = golden_bytes();
        assert_eq!(expected.len(), 151);
        let records = golden_records();
        assert_eq!(decode_policy_witness_bundle(&expected).unwrap(), records);
        let reversed = records.iter().rev().cloned().collect::<Vec<_>>();
        assert_eq!(encode_policy_witness_bundle(&reversed).unwrap(), expected);
        assert_eq!(reversed[0].role, PolicyScriptRole::Type, "encoding must not mutate caller order");
    }

    #[test]
    fn selection_requires_the_complete_hash_and_role() {
        let records = decode_policy_witness_bundle(&golden_bytes()).unwrap();
        let record = selected_record(&records, PolicyScriptRole::Type, &[0x11; 32]).unwrap();
        assert_eq!(record.tag, 0x01020304);
        assert!(record.args.is_empty());
        assert!(selected_record(&records, PolicyScriptRole::Lock, &[0x11; 32]).is_none());
        let mut wrong_hash = [0x11; 32];
        wrong_hash[31] = 0x12;
        assert!(selected_record(&records, PolicyScriptRole::Type, &wrong_hash).is_none());
    }

    #[test]
    fn same_hash_with_distinct_roles_and_tag_zero_are_structurally_valid() {
        let mut records = golden_records();
        records[1].script_hash = records[0].script_hash;
        records[1].tag = 0;
        assert_eq!(decode_policy_witness_bundle(&encode_policy_witness_bundle(&records).unwrap()).unwrap(), records);
    }

    #[test]
    fn record_count_and_duplicate_keys_are_bounded_before_encoding() {
        assert_eq!(encode_policy_witness_bundle(&[]), Err(PolicyWitnessError::RecordCount(0)));
        let mut records = (0..MAX_POLICY_WITNESS_RECORDS)
            .map(|index| PolicyWitnessRecord {
                role: PolicyScriptRole::Type,
                script_hash: [index as u8; 32],
                tag: index as u32,
                args: Vec::new(),
            })
            .collect::<Vec<_>>();
        let encoded = encode_policy_witness_bundle(&records).unwrap();
        assert_eq!(decode_policy_witness_bundle(&encoded).unwrap(), records);
        records.push(records[0].clone());
        assert_eq!(encode_policy_witness_bundle(&records), Err(PolicyWitnessError::RecordCount(9)));
        let mut duplicate = records[0].clone();
        duplicate.tag = u32::MAX;
        assert_eq!(encode_policy_witness_bundle(&[records[0].clone(), duplicate]), Err(PolicyWitnessError::DuplicateKey));
    }

    #[test]
    fn decoder_rejects_empty_and_excessive_count_headers() {
        let mut empty = POLICY_WITNESS_MAGIC.to_vec();
        empty.extend_from_slice(&4u32.to_le_bytes());
        assert_eq!(decode_policy_witness_bundle(&empty), Err(PolicyWitnessError::RecordCount(0)));
        let mut excessive = POLICY_WITNESS_MAGIC.to_vec();
        excessive.extend_from_slice(&40u32.to_le_bytes());
        excessive.extend_from_slice(&40u32.to_le_bytes());
        excessive.resize(48, 0);
        assert_eq!(decode_policy_witness_bundle(&excessive), Err(PolicyWitnessError::RecordCount(9)));
    }

    #[test]
    fn decoder_rejects_every_truncation_and_trailing_bytes() {
        let encoded = golden_bytes();
        for length in 0..encoded.len() {
            assert!(decode_policy_witness_bundle(&encoded[..length]).is_err(), "accepted truncation at {length}");
        }
        let mut trailing = encoded;
        trailing.push(0);
        assert!(decode_policy_witness_bundle(&trailing).is_err());
    }

    #[test]
    fn decoder_rejects_bad_magic_role_offsets_and_lengths() {
        let mut invalid = golden_bytes();
        invalid[6] = b'2';
        assert_eq!(decode_policy_witness_bundle(&invalid), Err(PolicyWitnessError::InvalidMagic));
        let mut invalid = golden_bytes();
        invalid[40] = 2;
        assert_eq!(decode_policy_witness_bundle(&invalid), Err(PolicyWitnessError::UnknownRole(2)));
        for offset in [8, 12, 16, 20, 24, 28, 32, 36, 77, 90, 94, 98, 102, 106, 147] {
            for value in [0, 1, u32::MAX] {
                let mut invalid = golden_bytes();
                // The last Bytes length is canonically zero; keep this a mutation.
                if offset == 147 && value == 0 {
                    continue;
                }
                replace_u32(&mut invalid, offset, value);
                assert!(decode_policy_witness_bundle(&invalid).is_err(), "accepted offset {offset} = {value}");
            }
        }
    }

    #[test]
    fn decoder_rejects_unsorted_and_duplicate_record_keys() {
        let canonical = golden_bytes();
        let mut unsorted = canonical[..20].to_vec();
        replace_u32(&mut unsorted, 16, 73); // 12-byte DynVec header + 61-byte Type record.
        unsorted.extend_from_slice(&canonical[90..]);
        unsorted.extend_from_slice(&canonical[20..90]);
        assert_eq!(decode_policy_witness_bundle(&unsorted), Err(PolicyWitnessError::NonCanonicalOrder));
        let mut duplicate = canonical;
        duplicate[110] = 0;
        duplicate[111..143].fill(0);
        assert_eq!(decode_policy_witness_bundle(&duplicate), Err(PolicyWitnessError::DuplicateKey));
    }

    #[test]
    fn nonempty_args_need_magic_but_variant_schema_remains_separate() {
        for args in [b"x".to_vec(), b"CSARGv2\0".to_vec(), vec![0; 8]] {
            let mut records = golden_records();
            records[0].args = args;
            assert_eq!(encode_policy_witness_bundle(&records), Err(PolicyWitnessError::InvalidArgsMagic));
        }
        let mut invalid = golden_bytes();
        invalid[81] = b'X';
        assert_eq!(decode_policy_witness_bundle(&invalid), Err(PolicyWitnessError::InvalidArgsMagic));
        let mut record = golden_records().remove(0);
        record.args = ENTRY_ARGS_MAGIC.to_vec();
        assert_eq!(decode_policy_witness_bundle(&encode_policy_witness_bundle(&[record.clone()]).unwrap()).unwrap(), vec![record]);
        // A selected no-payload action must reject this nonempty argument block.
    }

    #[test]
    fn exact_bundle_limit_is_accepted_and_one_more_byte_is_rejected() {
        let max_args = MAX_POLICY_WITNESS_BUNDLE_BYTES - POLICY_WITNESS_MAGIC.len() - 8 - RECORD_FIXED_BYTES;
        let mut record = golden_records().remove(0);
        record.args.resize(max_args, 0);
        let encoded = encode_policy_witness_bundle(&[record.clone()]).unwrap();
        assert_eq!(encoded.len(), MAX_POLICY_WITNESS_BUNDLE_BYTES);
        assert_eq!(decode_policy_witness_bundle(&encoded).unwrap(), vec![record.clone()]);
        record.args.push(0);
        assert_eq!(encode_policy_witness_bundle(&[record]), Err(PolicyWitnessError::SizeLimit));
        assert_eq!(decode_policy_witness_bundle(&vec![0; MAX_POLICY_WITNESS_BUNDLE_BYTES + 1]), Err(PolicyWitnessError::SizeLimit));
    }
}
