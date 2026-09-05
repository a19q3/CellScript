//! Bounded machine validation for fatal verification exits. This does not prove
//! arbitrary helper semantics or that every possible failure has been emitted.
use crate::{CheckerError, CheckerRejectionCode, EntryKind, ParsedElf, VerifiedLoweringRecord};
use std::collections::{BTreeMap, BTreeSet};

fn invalid(message: impl Into<String>) -> CheckerError {
    CheckerError::new(CheckerRejectionCode::V2414ControlFlowInvalid, message)
}

fn constant_word(register: u32, value: u32) -> u32 {
    (value << 20) | (register << 7) | 0x13
}

struct Machine<'a> {
    words: BTreeMap<u64, u32>,
    jumps: BTreeMap<u64, u64>,
    elf: &'a ParsedElf,
}

impl Machine<'_> {
    fn small_constant(&self, address: u64, register: u32, value: u32) -> Option<u64> {
        if self.words.get(&address) == Some(&constant_word(register, value)) {
            return Some(4);
        }
        // The internal assembler currently uses LUI+ADDI even for small li.
        if self.words.get(&address) == Some(&((register << 7) | 0x37))
            && self.words.get(&(address + 4)) == Some(&(constant_word(register, value) | (register << 15)))
        {
            return Some(8);
        }
        None
    }

    fn jump(&self, address: u64) -> Option<u64> {
        // The compiler's unconditional tail-jump contract is jal x0, offset.
        // Neither a call writing RA nor an arbitrary indirect jump is equivalent.
        let word = *self.words.get(&address)?;
        if word & 0xfff != 0x6f {
            return None;
        }
        self.jumps.get(&address).copied()
    }

    fn static_exit(&self, address: u64, code: u32, sink: u64) -> bool {
        self.small_constant(address, 10, code).is_some_and(|size| self.jump(address + size) == Some(sink))
    }

    fn exit_sink_size(&self, start: u64) -> Option<u64> {
        let size = self.small_constant(start, 17, 93)?;
        (self.words.get(&(start + size)) == Some(&0x73) && self.jump(start + size + 4) == Some(start)).then_some(size + 8)
    }
}

pub(crate) fn validate_verifier_failures<'a>(
    record: &'a VerifiedLoweringRecord,
    elf: &ParsedElf,
) -> Result<Option<&'a str>, CheckerError> {
    let machine = Machine {
        words: elf.instructions.iter().map(|instruction| (instruction.address, instruction.word)).collect(),
        jumps: elf.control_flow.iter().map(|edge| (edge.address, edge.target)).collect(),
        elf,
    };
    let blocks = record.blocks.iter().map(|block| (block.id.as_str(), block)).collect::<BTreeMap<_, _>>();
    let raw_sinks = machine.words.keys().copied().filter(|start| machine.exit_sink_size(*start).is_some()).collect::<BTreeSet<_>>();
    let mut targets = elf.control_flow.iter().map(|edge| edge.target).collect::<BTreeSet<_>>();
    targets.insert(elf.entry);
    let sinks = record.entries.iter().filter(|entry| entry.name == "__cellscript_abort").collect::<Vec<_>>();
    let sink = match sinks.as_slice() {
        [] if raw_sinks.is_empty() => None,
        [] => return Err(invalid("decoded verifier EXIT sink is missing its reserved entry contract")),
        [entry] => {
            let block = blocks.get(entry.entry_block.as_str()).ok_or_else(|| invalid("verifier EXIT sink has no block"))?;
            let start = block.range.start;
            let constant_size = machine.small_constant(start, 17, 93).unwrap_or(0);
            if entry.kind != EntryKind::Runtime
                || entry.frame_size_bytes != 0
                || entry.outgoing_argument_bytes != 0
                || constant_size == 0
                || raw_sinks.len() != 1
                || !raw_sinks.contains(&start)
                || block.range.end != start + constant_size + 8
                || record.blocks.iter().filter(|candidate| candidate.owner_entry == entry.id).count() != 1
                || machine.words.get(&(start + constant_size)) != Some(&0x73)
                || machine.jump(start + constant_size + 4) != Some(start)
                || targets.range((std::ops::Bound::Excluded(start), std::ops::Bound::Excluded(block.range.end))).next().is_some()
            {
                return Err(invalid("verifier EXIT sink is not the exact memory-free, non-returning current-process exit"));
            }
            Some(*block)
        }
        _ => return Err(invalid("duplicate verifier EXIT sinks")),
    };
    if !record.verifier_failure_exits.windows(2).all(|pair| pair[0].address < pair[1].address) {
        return Err(invalid("terminal verifier exits are not strictly address-sorted and unique"));
    }
    for exit in &record.verifier_failure_exits {
        let block = blocks.get(exit.block_id.as_str()).ok_or_else(|| invalid("terminal verifier exit has no owner block"))?;
        let size = machine.small_constant(exit.address, 10, exit.code as u32).unwrap_or(0);
        if !(1..=255).contains(&exit.code)
            || exit.name.is_empty()
            || !block.range.contains(exit.address)
            || size == 0
            || targets
                .range((std::ops::Bound::Excluded(exit.address), std::ops::Bound::Excluded(exit.address + size + 4)))
                .next()
                .is_some()
            || machine.small_constant(exit.address, 10, exit.code as u32).is_none_or(|size| exit.address + size + 4 > block.range.end)
            || !sink.is_some_and(|sink| machine.static_exit(exit.address, exit.code as u32, sink.range.start))
        {
            return Err(invalid(format!(
                "terminal verifier exit at {:#x} does not load its exact nonzero code and terminate",
                exit.address
            )));
        }
    }
    let exits = record.verifier_failure_exits.iter().map(|exit| (exit.address, exit)).collect::<BTreeMap<_, _>>();
    if let Some(sink) = sink {
        // Static terminal sites cannot be omitted merely by dropping their
        // sidecar record. Derive this inventory from the actual incoming jumps.
        for edge in &machine.elf.control_flow {
            if edge.target != sink.range.start || machine.jump(edge.address) != Some(sink.range.start) {
                continue;
            }
            let Some(last) = edge.address.checked_sub(4) else { continue };
            let Some(word) = machine.words.get(&last) else { continue };
            let base = word & 0x000f_ffff;
            let address = if base == constant_word(10, 0) {
                last
            } else if base == (constant_word(10, 0) | (10 << 15))
                && last.checked_sub(4).and_then(|address| machine.words.get(&address)) == Some(&((10 << 7) | 0x37))
            {
                last - 4
            } else {
                continue;
            };
            let code = word >> 20;
            if !(1..=255).contains(&code) || exits.get(&address).is_none_or(|exit| exit.code as u32 != code) {
                return Err(invalid("static verifier failure is zero, unsupported, or missing its mandatory record"));
            }
        }
    }
    let mut typed_bindings = BTreeMap::new();
    for block in &record.blocks {
        if let Some(id) = block.lowering_block_id {
            typed_bindings.entry((block.owner_entry.as_str(), id)).or_insert(block.range.start);
        }
    }
    // Every explicit IR/typed failure must reach a registered terminal site.
    // Follow only tail jumps, not claimed CFG edges, calls or arbitrary operations.
    for entry in &record.typed_semantics.entries {
        for typed in entry.blocks.iter().filter(|block| block.runtime_error.is_some()) {
            let error = typed.runtime_error.as_ref().expect("filtered runtime error");
            let mut address = *typed_bindings
                .get(&(entry.id.as_str(), typed.id))
                .ok_or_else(|| invalid("typed verifier failure has no machine binding"))?;
            let mut matched = false;
            for _ in 0..4 {
                if exits.get(&address).is_some_and(|exit| exit.code as u64 == error.code) {
                    matched = true;
                    break;
                }
                let Some(target) = machine.jump(address) else { break };
                address = target;
            }
            if !matched {
                return Err(invalid(format!(
                    "typed verifier failure '{}:{}' is not bound to its terminal machine exit",
                    entry.id, typed.id
                )));
            }
        }
    }
    Ok(sink.map(|block| block.id.as_str()))
}
