// Shared helpers for the matched Rust cost-corpus references. Each binary
// inlines its own trampoline and mirrors one CellScript scenario's checked
// scope exactly, under the audit's build profile.
#![allow(dead_code)]

use ckb_std::ckb_constants::{CellField, Source};
use ckb_std::error::SysError;
use ckb_std::syscalls::{load_cell_by_field, load_cell_data, load_witness};

pub fn field<const N: usize>(source: Source, kind: CellField) -> Result<[u8; N], i8> {
    let mut bytes = [0u8; N];
    let len = load_cell_by_field(&mut bytes, 0, 0, source,kind).map_err(|_| 10)?;
    if len != N {
        return Err(11);
    }
    Ok(bytes)
}

pub fn data<const N: usize>(source: Source) -> Result<[u8; N], i8> {
    let mut bytes = [0u8; N];
    let len = load_cell_data(&mut bytes, 0, 0, source).map_err(|_| 10)?;
    if len != N {
        return Err(11);
    }
    Ok(bytes)
}

pub fn data_at(source: Source, index: usize) -> Result<[u8; 8], i8> {
    let mut bytes = [0u8; 8];
    let len = load_cell_data(&mut bytes, 0, index, source).map_err(|_| 10)?;
    if len != 8 {
        return Err(11);
    }
    Ok(bytes)
}

pub fn u64_at(bytes: &[u8], at: usize) -> Result<u64, i8> {
    bytes
        .get(at..at + 8)
        .and_then(|slice| slice.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(12)
}

fn word(bytes: &[u8], at: usize) -> Result<usize, i8> {
    let value = bytes.get(at..at + 4).ok_or(12)?;
    Ok(u32::from_le_bytes(value.try_into().map_err(|_| 12)?) as usize)
}

/// Parse the shared WitnessArgs envelope and its single `CSARGv1` inner
/// argument, returning the 32-byte argument payload.
pub fn witness_address_arg() -> Result<[u8; 32], i8> {
    let mut buffer = [0u8; 4096];
    let len = match load_witness(&mut buffer, 0, 0, Source::GroupInput) {
        Ok(len) => len,
        Err(_) => {
            let mut capacity = [0; 8];
            match load_cell_by_field(&mut capacity, 0, 0, Source::GroupInput, CellField::Capacity) {
                Err(SysError::IndexOutOfBound) => {
                    load_witness(&mut buffer, 0, 0, Source::GroupOutput).map_err(|_| 12)?
                }
                _ => return Err(12),
            }
        }
    };
    let bytes = buffer.get(..len).ok_or(12)?;
    if len < 16 || word(bytes, 0)? != len || word(bytes, 4)? != 16 {
        return Err(12);
    }
    // Scan the WitnessArgs table fields and use whichever one carries the
    // CSARGv1 argument payload; the CellScript entry ABI may populate
    // sibling fields alongside it.
    let bounds = [16usize, word(bytes, 8)?, word(bytes, 12)?, len];
    for (start, end) in bounds.windows(2).map(|pair| (pair[0], pair[1])) {
        if start > end || end > len || end - start != 44 {
            continue;
        }
        let field = &bytes[start..end];
        if word(field, 0)? != 40 {
            continue;
        }
        let payload = &field[4..44];
        if &payload[..8] != b"CSARGv1\0" {
            continue;
        }
        let mut argument = [0u8; 32];
        argument.copy_from_slice(&payload[8..]);
        return Ok(argument);
    }
    Err(12)
}

