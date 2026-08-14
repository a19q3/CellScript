use super::*;

impl CodeGenerator {
    pub(super) fn emit_prologue(&mut self) {
        self.emit_large_addi("sp", "sp", -(self.frame_size as i64));
        self.emit_stack_store("ra", self.frame_size - 8);
        self.emit_stack_store("fp", self.frame_size - 16);
        self.emit_sp_addi("fp", self.frame_size);
    }

    pub(super) fn emit_epilogue(&mut self) {
        if let Some(function) = &self.current_function {
            self.emit(format!("j .L{}_epilogue", function));
            return;
        }
        self.emit_epilogue_body();
    }

    pub(super) fn emit_fail(&mut self, error: CellScriptRuntimeError) {
        if let Some(function) = self.current_function.clone() {
            self.fail_handler_codes.insert(error);
            self.emit(format!("j .L{}_fail_{}", function, error.code()));
            return;
        }
        self.emit_runtime_error_comment(error);
        self.emit(format!("li a0, {}", error.code()));
        self.emit_epilogue_body();
    }

    pub(super) fn emit_shared_epilogue(&mut self) {
        let Some(function) = self.current_function.clone() else {
            return;
        };
        let fail_codes = self.fail_handler_codes.iter().copied().collect::<Vec<_>>();
        for error in fail_codes {
            self.emit_label(&format!(".L{}_fail_{}", function, error.code()));
            self.emit_runtime_error_comment(error);
            self.emit(format!("li a0, {}", error.code()));
            self.emit(format!("j .L{}_epilogue", function));
        }
        self.emit_label(&format!(".L{}_epilogue", function));
        self.emit_epilogue_body();
    }

    pub(super) fn emit_runtime_error_comment(&mut self, error: CellScriptRuntimeError) {
        self.emit(format!("# cellscript runtime error {} {}", error.code(), error.name()));
    }

    pub(super) fn emit_epilogue_body(&mut self) {
        self.emit_stack_load("ra", self.frame_size - 8);
        self.emit_stack_load("fp", self.frame_size - 16);
        self.emit_large_addi("sp", "sp", self.frame_size as i64);
        self.emit("ret");
    }

    /// Emit `addi rd, rs1, imm` handling immediates that don't fit in 12 bits.
    pub(super) fn emit_large_addi(&mut self, rd: &str, rs1: &str, imm: i64) {
        if (-2048..=2047).contains(&imm) {
            self.emit(format!("addi {}, {}, {}", rd, rs1, imm));
        } else {
            let scratch = scratch_register_avoiding(&[rs1]);
            self.emit(format!("li {}, {}", scratch, imm));
            self.emit(format!("add {}, {}, {}", rd, rs1, scratch));
        }
    }

    pub(super) fn emit_memory_load_with_avoid(&mut self, opcode: &str, dst: &str, base: &str, offset: usize, avoid: &[&str]) {
        let offset = i64::try_from(offset).expect("memory offset should fit in i64");
        if small_signed_immediate(offset) {
            self.emit(format!("{} {}, {}({})", opcode, dst, offset, base));
        } else {
            let mut registers = Vec::with_capacity(2 + avoid.len());
            registers.push(dst);
            registers.push(base);
            registers.extend_from_slice(avoid);
            let scratch = scratch_register_avoiding(&registers);
            self.emit(format!("li {}, {}", scratch, offset));
            self.emit(format!("add {}, {}, {}", scratch, base, scratch));
            self.emit(format!("{} {}, 0({})", opcode, dst, scratch));
        }
    }

    pub(super) fn emit_memory_store_with_avoid(&mut self, opcode: &str, src: &str, base: &str, offset: usize, avoid: &[&str]) {
        let offset = i64::try_from(offset).expect("memory offset should fit in i64");
        if small_signed_immediate(offset) {
            self.emit(format!("{} {}, {}({})", opcode, src, offset, base));
        } else {
            let mut registers = Vec::with_capacity(2 + avoid.len());
            registers.push(src);
            registers.push(base);
            registers.extend_from_slice(avoid);
            let scratch = scratch_register_avoiding(&registers);
            self.emit(format!("li {}, {}", scratch, offset));
            self.emit(format!("add {}, {}, {}", scratch, base, scratch));
            self.emit(format!("{} {}, 0({})", opcode, src, scratch));
        }
    }

    /// Emit `ld rd, offset(sp)` through the centralized stack-offset gate.
    pub(super) fn emit_stack_load(&mut self, rd: &str, offset: usize) {
        self.emit_stack_access_with_avoid("ld", rd, offset, &[]);
    }

    /// Emit `ld rd, offset(sp)` without clobbering explicitly live registers.
    pub(super) fn emit_stack_load_with_avoid(&mut self, rd: &str, offset: usize, avoid: &[&str]) {
        self.emit_stack_access_with_avoid("ld", rd, offset, avoid);
    }

    /// Emit `lbu rd, offset(sp)` through the centralized stack-offset gate.
    pub(super) fn emit_stack_load_byte(&mut self, rd: &str, offset: usize) {
        self.emit_stack_access("lbu", rd, offset);
    }

    /// Emit `sd rs2, offset(sp)` through the centralized stack-offset gate.
    pub(super) fn emit_stack_store(&mut self, rs2: &str, offset: usize) {
        self.emit_stack_access_with_avoid("sd", rs2, offset, &[]);
    }

    /// Emit `sd rs2, offset(sp)` without clobbering explicitly live registers.
    pub(super) fn emit_stack_store_with_avoid(&mut self, rs2: &str, offset: usize, avoid: &[&str]) {
        self.emit_stack_access_with_avoid("sd", rs2, offset, avoid);
    }

    /// Emit `sb rs2, offset(sp)` through the centralized stack-offset gate.
    pub(super) fn emit_stack_store_byte(&mut self, rs2: &str, offset: usize) {
        self.emit_stack_access("sb", rs2, offset);
    }

    pub(super) fn emit_stack_access(&mut self, opcode: &str, register: &str, offset: usize) {
        self.emit_stack_access_with_avoid(opcode, register, offset, &[]);
    }

    pub(super) fn emit_stack_access_with_avoid(&mut self, opcode: &str, register: &str, offset: usize, avoid: &[&str]) {
        let offset = i64::try_from(offset).expect("stack offset should fit in i64");
        if small_signed_immediate(offset) {
            self.emit(format!("{} {}, {}(sp)", opcode, register, offset));
        } else {
            let mut live_registers = Vec::with_capacity(1 + avoid.len());
            live_registers.push(register);
            live_registers.extend_from_slice(avoid);
            let scratch = scratch_register_avoiding(&live_registers);
            self.emit(format!("li {}, {}", scratch, offset));
            self.emit(format!("add {}, sp, {}", scratch, scratch));
            self.emit(format!("{} {}, 0({})", opcode, register, scratch));
        }
    }

    /// Emit `addi rd, sp, offset` handling offsets that don't fit in 12 bits.
    pub(super) fn emit_sp_addi(&mut self, rd: &str, offset: usize) {
        if offset <= 2047 {
            self.emit(format!("addi {}, sp, {}", rd, offset));
        } else if rd == "sp" {
            self.emit_large_addi("sp", "sp", offset as i64);
        } else {
            self.emit(format!("li {}, {}", rd, offset));
            self.emit(format!("add {}, sp, {}", rd, rd));
        }
    }

    pub(super) fn prepare_function_layout(&mut self, body: &IrBody, params: &[IrParam]) {
        let mut max_var_id = None;
        let mut fixed_byte_locals = HashMap::<usize, usize>::new();
        let mut named_vars = BTreeSet::<String>::new();
        for param in params {
            self.record_var(&param.binding, &mut max_var_id);
        }
        for block in &body.blocks {
            for instruction in &block.instructions {
                self.record_instruction_var(instruction, &mut max_var_id);
                self.record_instruction_fixed_byte_local(instruction, &mut fixed_byte_locals);
                if let IrInstruction::StoreVar { name, .. } = instruction {
                    named_vars.insert(name.clone());
                }
            }
            self.record_terminator_var(&block.terminator, &mut max_var_id);
        }

        let locals_size = max_var_id.map(|id| (id + 1) * 8).unwrap_or(0);
        self.fixed_byte_local_offsets.clear();
        self.named_var_offsets.clear();
        self.cell_buffer_offsets.clear();
        self.cell_buffer_size_offsets.clear();
        self.dynamic_value_size_offsets.clear();
        self.empty_molecule_vector_vars.clear();
        self.constructed_byte_vectors.clear();
        self.constructed_byte_vector_roots.clear();
        self.verified_collection_construction_vectors.clear();
        self.output_type_hash_sources.clear();
        self.consume_order.clear();
        self.consume_indices.clear();
        self.consume_type_names.clear();
        self.consume_binding_ids.clear();
        self.read_ref_order.clear();
        self.read_ref_indices.clear();
        self.read_ref_param_ids.clear();
        self.read_ref_param_input_indices.clear();
        self.read_ref_param_dep_indices.clear();
        self.output_param_ids.clear();
        self.mutate_param_ids.clear();
        self.schema_pointer_size_offsets.clear();
        self.fixed_byte_param_size_offsets.clear();
        self.param_type_hash_pointer_offsets.clear();
        self.param_type_hash_size_offsets.clear();
        self.param_type_hash_sources.clear();
        self.u128_value_offsets.clear();
        self.collection_region_start = 0;
        self.next_collection_slot = 0;

        let schema_param_ids =
            params.iter().filter(|param| named_type_name(&param.ty).is_some()).map(|param| param.binding.id).collect::<BTreeSet<_>>();
        let mut param_type_hash_ids = BTreeSet::new();
        for block in &body.blocks {
            for instruction in &block.instructions {
                if let IrInstruction::TypeHash { dest, operand: IrOperand::Var(var) } = instruction
                    && schema_param_ids.contains(&var.id)
                {
                    param_type_hash_ids.insert(var.id);
                    self.param_type_hash_sources.insert(dest.id, var.id);
                }
            }
        }

        let mut next_cell_slot = locals_size;
        let mut fixed_byte_locals = fixed_byte_locals.into_iter().collect::<Vec<_>>();
        fixed_byte_locals.sort_unstable_by_key(|(var_id, _)| *var_id);
        for (var_id, width) in fixed_byte_locals {
            next_cell_slot = align_up(next_cell_slot, 8);
            self.fixed_byte_local_offsets.insert(var_id, next_cell_slot);
            next_cell_slot += align_up(width, 8);
        }
        for name in named_vars {
            next_cell_slot = align_up(next_cell_slot, 8);
            self.named_var_offsets.insert(name, next_cell_slot);
            next_cell_slot += 8;
        }
        for param in params {
            if param.source == ParamSource::Output {
                self.output_param_ids.insert(param.name.clone(), param.binding.id);
                self.schema_pointer_size_offsets.insert(param.binding.id, next_cell_slot);
                self.cell_buffer_size_offsets.insert(param.binding.id, next_cell_slot);
                self.cell_buffer_offsets.insert(param.binding.id, next_cell_slot + 8);
                next_cell_slot += RUNTIME_CELL_SLOT_SIZE;
                continue;
            }
            if named_type_name(&param.ty).is_some() {
                self.schema_pointer_size_offsets.insert(param.binding.id, next_cell_slot);
                next_cell_slot += 8;
            } else if fixed_byte_pointer_param_width(&param.ty).is_some() || fixed_aggregate_pointer_param_width(&param.ty).is_some() {
                self.fixed_byte_param_size_offsets.insert(param.binding.id, next_cell_slot);
                next_cell_slot += 8;
            }
        }
        for param in params {
            if param_type_hash_ids.contains(&param.binding.id) {
                self.param_type_hash_pointer_offsets.insert(param.binding.id, next_cell_slot);
                next_cell_slot += 8;
                self.param_type_hash_size_offsets.insert(param.binding.id, next_cell_slot);
                next_cell_slot += 8;
            }
        }

        if self.bind_readonly_schema_params {
            let consumed_param_names = body.consume_set.iter().map(|pattern| pattern.binding.as_str()).collect::<BTreeSet<_>>();
            let mutate_param_names = body.mutate_set.iter().map(|pattern| pattern.binding.as_str()).collect::<BTreeSet<_>>();
            let read_ref_indices_by_binding =
                body.read_refs.iter().enumerate().map(|(index, pattern)| (pattern.binding.as_str(), index)).collect::<HashMap<_, _>>();
            let mut read_ref_param_index = 0usize;
            for param in params {
                if matches!(param.source, ParamSource::Output | ParamSource::LockArgs) {
                    continue;
                }
                if !self.param_is_runtime_bound(param) {
                    continue;
                }
                if mutate_param_names.contains(param.name.as_str()) || consumed_param_names.contains(param.name.as_str()) {
                    continue;
                }
                self.read_ref_param_ids.insert(param.name.clone(), param.binding.id);
                if let Some(dep_index) = read_ref_indices_by_binding.get(param.name.as_str()).copied() {
                    self.read_ref_param_dep_indices.insert(param.binding.id, dep_index);
                } else {
                    let input_index = body.consume_set.len() + body.mutate_set.len() + read_ref_param_index;
                    self.read_ref_param_input_indices.insert(param.binding.id, input_index);
                    read_ref_param_index += 1;
                }
                self.schema_pointer_size_offsets.insert(param.binding.id, next_cell_slot);
                self.cell_buffer_size_offsets.insert(param.binding.id, next_cell_slot);
                self.cell_buffer_offsets.insert(param.binding.id, next_cell_slot + 8);
                next_cell_slot += RUNTIME_CELL_SLOT_SIZE;
            }
        }

        for pattern in &body.mutate_set {
            let Some(param) = params.iter().find(|param| param.name == pattern.binding) else {
                continue;
            };
            self.mutate_param_ids.insert(pattern.binding.clone(), param.binding.id);
            self.consume_type_names.insert(param.binding.id, pattern.ty.clone());
            self.consume_binding_ids.insert(pattern.binding.clone(), param.binding.id);
            self.consume_indices.insert(param.binding.id, pattern.input_index);
            self.schema_pointer_size_offsets.insert(param.binding.id, next_cell_slot);
            self.cell_buffer_size_offsets.insert(param.binding.id, next_cell_slot);
            self.cell_buffer_offsets.insert(param.binding.id, next_cell_slot + 8);
            next_cell_slot += RUNTIME_CELL_SLOT_SIZE;
        }

        let consume_pattern_indices =
            body.consume_set.iter().enumerate().map(|(index, pattern)| (pattern.binding.as_str(), index)).collect::<HashMap<_, _>>();
        for pattern in &body.consume_set {
            let Some(param) = params.iter().find(|param| param.name == pattern.binding) else {
                continue;
            };
            if self.consume_binding_ids.contains_key(&pattern.binding) {
                continue;
            }
            if let Some(type_name) = named_type_name(&param.ty) {
                self.consume_type_names.insert(param.binding.id, type_name.to_string());
            }
            self.consume_binding_ids.insert(pattern.binding.clone(), param.binding.id);
            self.schema_pointer_size_offsets.insert(param.binding.id, next_cell_slot);
            self.cell_buffer_size_offsets.insert(param.binding.id, next_cell_slot);
            self.cell_buffer_offsets.insert(param.binding.id, next_cell_slot + 8);
            self.consume_order.push(param.binding.id);
            self.consume_indices.insert(param.binding.id, consume_pattern_indices.get(pattern.binding.as_str()).copied().unwrap_or(0));
            next_cell_slot += RUNTIME_CELL_SLOT_SIZE;
        }
        for block in &body.blocks {
            for instruction in &block.instructions {
                if let Some(var) = consumed_operand_var(instruction) {
                    if self.consume_binding_ids.contains_key(&var.name) {
                        continue;
                    }
                    if let Some(type_name) = named_type_name(&var.ty) {
                        self.consume_type_names.insert(var.id, type_name.to_string());
                    }
                    self.consume_binding_ids.insert(var.name.clone(), var.id);
                    self.schema_pointer_size_offsets.insert(var.id, next_cell_slot);
                    self.cell_buffer_size_offsets.insert(var.id, next_cell_slot);
                    self.cell_buffer_offsets.insert(var.id, next_cell_slot + 8);
                    self.consume_order.push(var.id);
                    self.consume_indices.insert(
                        var.id,
                        consume_pattern_indices.get(var.name.as_str()).copied().unwrap_or(self.consume_order.len() - 1),
                    );
                    next_cell_slot += RUNTIME_CELL_SLOT_SIZE;
                }
            }
        }

        let mut read_ref_index = 0usize;
        for block in &body.blocks {
            for instruction in &block.instructions {
                if let IrInstruction::ReadRef { dest, .. } = instruction {
                    self.cell_buffer_size_offsets.insert(dest.id, next_cell_slot);
                    self.cell_buffer_offsets.insert(dest.id, next_cell_slot + 8);
                    self.read_ref_order.push(dest.id);
                    self.read_ref_indices.insert(dest.id, read_ref_index);
                    next_cell_slot += RUNTIME_CELL_SLOT_SIZE;
                    read_ref_index += 1;
                }
            }
        }

        let mut create_dest_outputs = HashMap::new();
        let mut next_create_output_index =
            body.create_set.iter().position(|pattern| pattern.operation == "create").unwrap_or(body.create_set.len());
        for block in &body.blocks {
            for instruction in &block.instructions {
                match instruction {
                    IrInstruction::FieldAccess { dest, obj: IrOperand::Var(obj), field } => {
                        if named_type_name(&dest.ty).is_some()
                            && named_type_name(&obj.ty)
                                .and_then(|type_name| self.type_layouts.get(type_name))
                                .and_then(|fields| fields.get(field))
                                .is_some_and(|layout| {
                                    layout_fixed_byte_width(layout).is_none()
                                        && molecule_vector_element_fixed_width(
                                            &layout.ty,
                                            &self.type_fixed_sizes,
                                            &self.enum_fixed_sizes,
                                        )
                                        .is_some()
                                })
                        {
                            self.dynamic_value_size_offsets.insert(dest.id, next_cell_slot);
                            next_cell_slot += 8;
                        }
                    }
                    IrInstruction::Create { dest, pattern } => {
                        let output_index = if pattern.operation == "create" {
                            let output_index = next_create_output_index;
                            next_create_output_index += 1;
                            Some(output_index)
                        } else {
                            Self::create_output_index(body, &pattern.operation, &pattern.binding, &pattern.ty)
                        };
                        if let Some(output_index) = output_index {
                            create_dest_outputs.insert(dest.id, output_index);
                        }
                    }
                    IrInstruction::CreateUnique { dest, pattern, .. } | IrInstruction::ReplaceUnique { dest, pattern, .. } => {
                        if let Some(output_index) = Self::create_output_index(body, &pattern.operation, &pattern.binding, &pattern.ty)
                        {
                            create_dest_outputs.insert(dest.id, output_index);
                        }
                    }
                    IrInstruction::Transfer { dest, .. } => {
                        if let Some(output_index) = Self::create_output_index_for_dest(body, "transfer", dest) {
                            create_dest_outputs.insert(dest.id, output_index);
                        }
                    }
                    IrInstruction::Claim { dest, .. } => {
                        if let Some(output_index) = Self::create_output_index_for_dest(body, "claim", dest) {
                            create_dest_outputs.insert(dest.id, output_index);
                        }
                    }
                    IrInstruction::Settle { dest, .. } => {
                        if let Some(output_index) = Self::create_output_index_for_dest(body, "settle", dest) {
                            create_dest_outputs.insert(dest.id, output_index);
                        }
                    }
                    IrInstruction::TypeHash { dest, operand: IrOperand::Var(var) } => {
                        if let Some(output_index) = create_dest_outputs.get(&var.id).copied() {
                            self.output_type_hash_sources.insert(dest.id, output_index);
                            self.cell_buffer_size_offsets.insert(dest.id, next_cell_slot);
                            self.cell_buffer_offsets.insert(dest.id, next_cell_slot + 8);
                            next_cell_slot += RUNTIME_CELL_SLOT_SIZE;
                        } else if self.consume_indices.contains_key(&var.id)
                            || self.read_ref_indices.contains_key(&var.id)
                            || self.read_ref_param_input_indices.contains_key(&var.id)
                        {
                            self.cell_buffer_size_offsets.insert(dest.id, next_cell_slot);
                            self.cell_buffer_offsets.insert(dest.id, next_cell_slot + 8);
                            next_cell_slot += RUNTIME_CELL_SLOT_SIZE;
                        }
                    }
                    IrInstruction::Call { dest: Some(dest), func, args }
                        if func == "__ckb_current_script_hash" && args.is_empty() && dest.ty == IrType::Hash =>
                    {
                        self.cell_buffer_size_offsets.insert(dest.id, next_cell_slot);
                        self.cell_buffer_offsets.insert(dest.id, next_cell_slot + 8);
                        next_cell_slot += RUNTIME_CELL_SLOT_SIZE;
                    }
                    IrInstruction::Call { dest: Some(dest), func, args }
                        if matches!(
                            func.as_str(),
                            "__ckb_input_out_point_tx_hash"
                                | "__ckb_cell_lock_hash"
                                | "__ckb_cell_type_hash"
                                | "__ckb_cell_data_hash"
                                | "__ckb_cell_data_hash_at"
                                | "__ckb_cell_lock_code_hash"
                                | "__ckb_cell_type_code_hash"
                                | "__ckb_cell_lock_args_hash"
                                | "__ckb_cell_type_args_hash"
                        ) && (args.len() == 1 || (func == "__ckb_cell_data_hash_at" && args.len() == 2))
                            && dest.ty == IrType::Hash =>
                    {
                        self.cell_buffer_size_offsets.insert(dest.id, next_cell_slot);
                        self.cell_buffer_offsets.insert(dest.id, next_cell_slot + 8);
                        next_cell_slot += RUNTIME_CELL_SLOT_SIZE;
                    }
                    IrInstruction::Call { dest: Some(dest), func, args }
                        if matches!(
                            func.as_str(),
                            "__ckb_witness_raw" | "__ckb_witness_lock" | "__ckb_witness_input_type" | "__ckb_witness_output_type"
                        ) && args.len() == 1
                            && dest.ty == IrType::Hash =>
                    {
                        self.cell_buffer_size_offsets.insert(dest.id, next_cell_slot);
                        self.cell_buffer_offsets.insert(dest.id, next_cell_slot + 8);
                        next_cell_slot += RUNTIME_CELL_SLOT_SIZE;
                    }
                    _ => {}
                }
            }
        }

        let mut u128_value_ids = BTreeSet::new();
        for param in params {
            if param.ty == IrType::U128 {
                u128_value_ids.insert(param.binding.id);
            }
        }
        for block in &body.blocks {
            for instruction in &block.instructions {
                self.collect_u128_instruction_vars(instruction, &mut u128_value_ids);
            }
            self.collect_u128_terminator_vars(&block.terminator, &mut u128_value_ids);
        }
        for var_id in u128_value_ids {
            self.u128_value_offsets.insert(var_id, next_cell_slot);
            next_cell_slot += 16;
        }

        let collection_slot_size = 8 + RUNTIME_COLLECTION_BUFFER_SIZE;
        let collection_count = body
            .blocks
            .iter()
            .flat_map(|block| block.instructions.iter())
            .filter(|instruction| matches!(instruction, IrInstruction::CollectionNew { .. }))
            .count();
        self.collection_region_start = next_cell_slot;
        next_cell_slot += collection_count * collection_slot_size;

        self.frame_size = align_frame(next_cell_slot + RUNTIME_EXPR_TEMP_SIZE + RUNTIME_SCRATCH_SIZE + 16);
    }

    pub(super) fn runtime_expr_temp_offset(&self, depth: usize) -> usize {
        debug_assert!(depth < RUNTIME_EXPR_TEMP_SLOTS);
        self.runtime_scratch_size_offset() - RUNTIME_EXPR_TEMP_SIZE + depth * 8
    }

    pub(super) fn checked_runtime_expr_temp_offset(&self, depth: usize) -> Option<usize> {
        (depth < RUNTIME_EXPR_TEMP_SLOTS).then(|| self.runtime_expr_temp_offset(depth))
    }

    pub(super) fn runtime_scratch_size_offset(&self) -> usize {
        self.frame_size - 16 - RUNTIME_SCRATCH_SIZE
    }

    pub(super) fn runtime_scratch_buffer_offset(&self) -> usize {
        self.runtime_scratch_size_offset() + 8
    }

    pub(super) fn runtime_scratch2_size_offset(&self) -> usize {
        self.runtime_scratch_size_offset() + RUNTIME_SCRATCH_SLOT_SIZE
    }

    pub(super) fn runtime_scratch2_buffer_offset(&self) -> usize {
        self.runtime_scratch2_size_offset() + 8
    }

    pub(super) fn emit_store_data_args_at(&mut self, max_bytes: usize, size_offset: usize, buffer_offset: usize) {
        self.emit(format!("li t0, {}", max_bytes));
        self.emit_stack_store("t0", size_offset);
        self.emit_sp_addi("a0", buffer_offset);
        self.emit_sp_addi("a1", size_offset);
        self.emit("li a2, 0");
    }

    pub(super) fn emit_load_cell_data_syscall(&mut self, reason: &str, source: u64, index: usize) {
        let size_offset = self.runtime_scratch_size_offset();
        let buffer_offset = self.runtime_scratch_buffer_offset();
        self.emit_load_cell_data_syscall_to_offsets(reason, source, index, size_offset, buffer_offset, RUNTIME_SCRATCH_BUFFER_SIZE);
    }

    pub(super) fn emit_load_cell_data_syscall_to_offsets(
        &mut self,
        reason: &str,
        source: u64,
        index: usize,
        size_offset: usize,
        buffer_offset: usize,
        max_bytes: usize,
    ) {
        self.emit(format!("# cellscript abi: LOAD_CELL_DATA reason={} source={} index={}", reason, ckb_source_name(source), index));
        self.emit_store_data_args_at(max_bytes, size_offset, buffer_offset);
        self.emit(format!("li a3, {}", index));
        self.emit(format!("li a4, {}", source));
        self.emit(format!("li a7, {}", self.runtime_abi().load_cell_data));
        self.emit("ecall");
        self.emit("# a0 = CKB syscall return code");
    }

    pub(super) fn emit_load_witness_syscall_to_offsets(
        &mut self,
        reason: &str,
        source: u64,
        index: usize,
        size_offset: usize,
        buffer_offset: usize,
        max_bytes: usize,
    ) {
        self.emit(format!("# cellscript abi: LOAD_WITNESS reason={} source={} index={}", reason, ckb_source_name(source), index));
        self.emit_store_data_args_at(max_bytes, size_offset, buffer_offset);
        self.emit(format!("li a3, {}", index));
        self.emit(format!("li a4, {}", source));
        self.emit(format!("li a7, {}", self.runtime_abi().load_witness));
        self.emit("ecall");
        self.emit("# a0 = CKB syscall return code");
    }

    pub(super) fn emit_load_script_syscall_to_offsets(
        &mut self,
        reason: &str,
        size_offset: usize,
        buffer_offset: usize,
        max_bytes: usize,
    ) {
        self.emit(format!("# cellscript abi: LOAD_SCRIPT reason={}", reason));
        self.emit_store_data_args_at(max_bytes, size_offset, buffer_offset);
        self.emit(format!("li a7, {}", self.runtime_abi().load_script));
        self.emit("ecall");
        self.emit("# a0 = CKB syscall return code");
    }

    pub(super) fn emit_load_cell_by_field_syscall_to_offsets(
        &mut self,
        reason: &str,
        source: u64,
        index: usize,
        field: u64,
        size_offset: usize,
        buffer_offset: usize,
        max_bytes: usize,
    ) {
        self.emit(format!(
            "# cellscript abi: LOAD_CELL_BY_FIELD reason={} source={} index={} field={}",
            reason,
            ckb_source_name(source),
            index,
            field
        ));
        self.emit_store_data_args_at(max_bytes, size_offset, buffer_offset);
        self.emit(format!("li a3, {}", index));
        self.emit(format!("li a4, {}", source));
        self.emit(format!("li a5, {}", field));
        self.emit(format!("li a7, {}", self.runtime_abi().load_cell_by_field));
        self.emit("ecall");
        self.emit("# a0 = CKB syscall return code");
    }

    pub(super) fn emit_load_cell_by_field_syscall_to_offsets_dynamic_index(
        &mut self,
        reason: &str,
        source: u64,
        index_reg: &str,
        field: u64,
        size_offset: usize,
        buffer_offset: usize,
        max_bytes: usize,
    ) {
        self.emit(format!(
            "# cellscript abi: LOAD_CELL_BY_FIELD reason={} source={} index={} field={}",
            reason,
            ckb_source_name(source),
            index_reg,
            field
        ));
        self.emit_store_data_args_at(max_bytes, size_offset, buffer_offset);
        self.emit(format!("addi a3, {}, 0", index_reg));
        self.emit(format!("li a4, {}", source));
        self.emit(format!("li a5, {}", field));
        self.emit(format!("li a7, {}", self.runtime_abi().load_cell_by_field));
        self.emit("ecall");
        self.emit("# a0 = CKB syscall return code");
    }

    pub(super) fn emit_return_on_syscall_error(&mut self, error: CellScriptRuntimeError) {
        let ok_label = self.fresh_label("ckb_syscall_ok");
        self.emit(format!("beqz a0, {}", ok_label));
        self.emit_fail(error);
        self.emit_label(&ok_label);
    }

    pub(super) fn emit_param_spills(&mut self, params: &[IrParam]) -> Result<()> {
        let mut abi_index = 0usize;
        for param in params {
            if named_type_name(&param.ty).is_some() {
                self.emit(format!(
                    "# cellscript abi: schema param {} pointer={} length={}",
                    param.name,
                    abi_arg_label(abi_index),
                    abi_arg_label(abi_index + 1)
                ));
                self.emit_spill_abi_arg(abi_index, param.binding.id * 8);
                if let Some(size_offset) = self.schema_pointer_size_offsets.get(&param.binding.id).copied() {
                    self.emit_spill_abi_arg(abi_index + 1, size_offset);
                }
                abi_index += 2;
                if let (Some(pointer_offset), Some(size_offset)) = (
                    self.param_type_hash_pointer_offsets.get(&param.binding.id).copied(),
                    self.param_type_hash_size_offsets.get(&param.binding.id).copied(),
                ) {
                    self.emit(format!(
                        "# cellscript abi: schema param {} type_hash pointer={} length={} size=32",
                        param.name,
                        abi_arg_label(abi_index),
                        abi_arg_label(abi_index + 1)
                    ));
                    self.emit_spill_abi_arg(abi_index, pointer_offset);
                    self.emit_spill_abi_arg(abi_index + 1, size_offset);
                    abi_index += 2;
                }
            } else if let Some(width) = fixed_byte_pointer_param_width(&param.ty) {
                self.emit(format!(
                    "# cellscript abi: fixed-byte param {} pointer={} length={} size={}",
                    param.name,
                    abi_arg_label(abi_index),
                    abi_arg_label(abi_index + 1),
                    width
                ));
                self.emit_spill_abi_arg(abi_index, param.binding.id * 8);
                if let Some(size_offset) = self.fixed_byte_param_size_offsets.get(&param.binding.id).copied() {
                    self.emit_spill_abi_arg(abi_index + 1, size_offset);
                }
                abi_index += 2;
            } else if let Some(width) = fixed_aggregate_pointer_param_width(&param.ty) {
                self.emit(format!(
                    "# cellscript abi: fixed-aggregate param {} pointer={} length={} size={}",
                    param.name,
                    abi_arg_label(abi_index),
                    abi_arg_label(abi_index + 1),
                    width
                ));
                self.emit_spill_abi_arg(abi_index, param.binding.id * 8);
                if let Some(size_offset) = self.fixed_byte_param_size_offsets.get(&param.binding.id).copied() {
                    self.emit_spill_abi_arg(abi_index + 1, size_offset);
                }
                abi_index += 2;
            } else {
                self.emit_spill_abi_arg(abi_index, param.binding.id * 8);
                abi_index += 1;
            }
        }

        Ok(())
    }

    pub(super) fn emit_spill_abi_arg(&mut self, abi_index: usize, stack_offset: usize) {
        if abi_index < 8 {
            self.emit_stack_store(&format!("a{}", abi_index), stack_offset);
        } else {
            let caller_stack_offset = (abi_index - 8) * 8;
            self.emit(format!("# cellscript abi: arg{} loaded from caller stack +{}", abi_index, caller_stack_offset));
            self.emit(format!("ld t0, {}(fp)", caller_stack_offset));
            self.emit_stack_store("t0", stack_offset);
        }
    }

    pub(super) fn record_instruction_var(&self, instruction: &IrInstruction, max_var_id: &mut Option<usize>) {
        match instruction {
            IrInstruction::LoadConst { dest, .. }
            | IrInstruction::LoadVar { dest, .. }
            | IrInstruction::Unary { dest, .. }
            | IrInstruction::FieldAccess { dest, .. }
            | IrInstruction::Index { dest, .. }
            | IrInstruction::Length { dest, .. }
            | IrInstruction::TypeHash { dest, .. }
            | IrInstruction::Create { dest, .. }
            | IrInstruction::CreateUnique { dest, .. }
            | IrInstruction::ReadRef { dest, .. } => self.record_var(dest, max_var_id),
            IrInstruction::CollectionNew { dest, capacity, .. } => {
                self.record_var(dest, max_var_id);
                if let Some(capacity) = capacity {
                    self.record_operand(capacity, max_var_id);
                }
            }
            IrInstruction::Move { dest, src } => {
                self.record_var(dest, max_var_id);
                self.record_operand(src, max_var_id);
            }
            IrInstruction::Tuple { dest, fields } => {
                self.record_var(dest, max_var_id);
                for field in fields {
                    self.record_operand(field, max_var_id);
                }
            }
            IrInstruction::EnumConstruct { dest, fields, .. } => {
                self.record_var(dest, max_var_id);
                for field in fields {
                    self.record_operand(field, max_var_id);
                }
            }
            IrInstruction::EnumTag { dest, operand, .. } | IrInstruction::EnumPayload { dest, operand, .. } => {
                self.record_var(dest, max_var_id);
                self.record_operand(operand, max_var_id);
            }
            IrInstruction::Binary { dest, left, right, .. } => {
                self.record_var(dest, max_var_id);
                self.record_operand(left, max_var_id);
                self.record_operand(right, max_var_id);
            }
            IrInstruction::StoreVar { src, .. } => self.record_operand(src, max_var_id),
            IrInstruction::Call { dest, args, .. } => {
                if let Some(dest) = dest {
                    self.record_var(dest, max_var_id);
                }
                for arg in args {
                    self.record_operand(arg, max_var_id);
                }
            }
            IrInstruction::Consume { operand } | IrInstruction::Destroy { operand, policy: _ } => {
                self.record_operand(operand, max_var_id)
            }
            IrInstruction::Transfer { dest, operand, to } => {
                self.record_var(dest, max_var_id);
                self.record_operand(operand, max_var_id);
                self.record_operand(to, max_var_id);
            }
            IrInstruction::Claim { dest, receipt } => {
                self.record_var(dest, max_var_id);
                self.record_operand(receipt, max_var_id);
            }
            IrInstruction::Settle { dest, operand } => {
                self.record_var(dest, max_var_id);
                self.record_operand(operand, max_var_id)
            }
            IrInstruction::ReplaceUnique { dest, operand, .. } => {
                self.record_var(dest, max_var_id);
                self.record_operand(operand, max_var_id)
            }
            IrInstruction::CellMetadataEquality { left, right, .. } => {
                self.record_operand(left, max_var_id);
                self.record_operand(right, max_var_id);
            }
            IrInstruction::CollectionPush { collection, value } => {
                self.record_operand(collection, max_var_id);
                self.record_operand(value, max_var_id);
            }
            IrInstruction::CollectionCapacity { dest, collection } => {
                self.record_var(dest, max_var_id);
                self.record_operand(collection, max_var_id);
            }
            IrInstruction::CollectionExtend { collection, slice } => {
                self.record_operand(collection, max_var_id);
                self.record_operand(slice, max_var_id);
            }
            IrInstruction::CollectionClear { collection } => {
                self.record_operand(collection, max_var_id);
            }
            IrInstruction::CollectionReverse { collection } => {
                self.record_operand(collection, max_var_id);
            }
            IrInstruction::CollectionTruncate { collection, len } => {
                self.record_operand(collection, max_var_id);
                self.record_operand(len, max_var_id);
            }
            IrInstruction::CollectionSwap { collection, left, right } => {
                self.record_operand(collection, max_var_id);
                self.record_operand(left, max_var_id);
                self.record_operand(right, max_var_id);
            }
            IrInstruction::CollectionContains { dest, collection, value } => {
                self.record_var(dest, max_var_id);
                self.record_operand(collection, max_var_id);
                self.record_operand(value, max_var_id);
            }
            IrInstruction::CollectionRemove { dest, collection, index } => {
                self.record_var(dest, max_var_id);
                self.record_operand(collection, max_var_id);
                self.record_operand(index, max_var_id);
            }
            IrInstruction::CollectionInsert { collection, index, value } => {
                self.record_operand(collection, max_var_id);
                self.record_operand(index, max_var_id);
                self.record_operand(value, max_var_id);
            }
            IrInstruction::CollectionSet { collection, index, value } => {
                self.record_operand(collection, max_var_id);
                self.record_operand(index, max_var_id);
                self.record_operand(value, max_var_id);
            }
            IrInstruction::CollectionPop { dest, collection } => {
                self.record_var(dest, max_var_id);
                self.record_operand(collection, max_var_id);
            }
        }
    }

    pub(super) fn record_instruction_fixed_byte_local(&self, instruction: &IrInstruction, offsets: &mut HashMap<usize, usize>) {
        let record = |offsets: &mut HashMap<usize, usize>, var: &IrVar| {
            if var.ty == IrType::U128 {
                offsets.insert(var.id, 16);
            }
            if let Some(width) = fixed_byte_width(&var.ty, type_static_length(&var.ty)).filter(|width| *width > 8) {
                offsets.insert(var.id, width);
            }
            if let Some(width) = self.fixed_named_type_width(&var.ty) {
                offsets.insert(var.id, width);
            }
        };

        match instruction {
            IrInstruction::LoadConst { dest, .. }
            | IrInstruction::LoadVar { dest, .. }
            | IrInstruction::Unary { dest, .. }
            | IrInstruction::FieldAccess { dest, .. }
            | IrInstruction::Index { dest, .. }
            | IrInstruction::Length { dest, .. }
            | IrInstruction::TypeHash { dest, .. }
            | IrInstruction::Create { dest, .. }
            | IrInstruction::CreateUnique { dest, .. }
            | IrInstruction::ReplaceUnique { dest, .. }
            | IrInstruction::Transfer { dest, .. }
            | IrInstruction::Claim { dest, .. }
            | IrInstruction::Settle { dest, .. }
            | IrInstruction::ReadRef { dest, .. }
            | IrInstruction::CollectionCapacity { dest, .. }
            | IrInstruction::CollectionContains { dest, .. }
            | IrInstruction::CollectionRemove { dest, .. }
            | IrInstruction::CollectionPop { dest, .. }
            | IrInstruction::CollectionNew { dest, .. }
            | IrInstruction::Move { dest, .. }
            | IrInstruction::Tuple { dest, .. }
            | IrInstruction::EnumConstruct { dest, .. }
            | IrInstruction::EnumTag { dest, .. }
            | IrInstruction::Binary { dest, .. } => record(offsets, dest),
            IrInstruction::EnumPayload { dest, enum_name, variant, field_index, .. } => {
                record(offsets, dest);
                if let Some(field) = self
                    .enum_layouts
                    .get(enum_name)
                    .and_then(|layout| layout.variants.iter().find(|candidate| candidate.name == *variant))
                    .and_then(|variant| variant.fields.get(*field_index))
                    && !field.linear
                    && !is_fixed_scalar_ir_type(&field.ty)
                    && field.width > 0
                {
                    offsets.insert(dest.id, field.width);
                }
            }
            IrInstruction::Call { dest, func, .. } => {
                if let Some(dest) = dest {
                    if is_ckb_fixed_hash_helper(func) && dest.ty == IrType::Hash {
                        offsets.insert(dest.id, 32);
                    }
                    record(offsets, dest);
                }
            }
            IrInstruction::StoreVar { .. }
            | IrInstruction::Consume { .. }
            | IrInstruction::Destroy { .. }
            | IrInstruction::CellMetadataEquality { .. }
            | IrInstruction::CollectionPush { .. }
            | IrInstruction::CollectionExtend { .. }
            | IrInstruction::CollectionClear { .. }
            | IrInstruction::CollectionReverse { .. }
            | IrInstruction::CollectionTruncate { .. }
            | IrInstruction::CollectionSwap { .. }
            | IrInstruction::CollectionInsert { .. }
            | IrInstruction::CollectionSet { .. } => {}
        }
    }

    pub(super) fn record_terminator_var(&self, terminator: &IrTerminator, max_var_id: &mut Option<usize>) {
        match terminator {
            IrTerminator::Return(Some(operand)) | IrTerminator::Branch { cond: operand, .. } => {
                self.record_operand(operand, max_var_id)
            }
            IrTerminator::Return(None) | IrTerminator::Jump(_) => {}
        }
    }

    pub(super) fn collect_u128_instruction_vars(&self, instruction: &IrInstruction, out: &mut BTreeSet<usize>) {
        match instruction {
            IrInstruction::LoadConst { dest, .. }
            | IrInstruction::LoadVar { dest, .. }
            | IrInstruction::Unary { dest, .. }
            | IrInstruction::FieldAccess { dest, .. }
            | IrInstruction::Index { dest, .. }
            | IrInstruction::Length { dest, .. }
            | IrInstruction::TypeHash { dest, .. }
            | IrInstruction::Create { dest, .. }
            | IrInstruction::CreateUnique { dest, .. }
            | IrInstruction::ReplaceUnique { dest, .. }
            | IrInstruction::Claim { dest, .. }
            | IrInstruction::ReadRef { dest, .. }
            | IrInstruction::CollectionCapacity { dest, .. }
            | IrInstruction::CollectionContains { dest, .. }
            | IrInstruction::CollectionRemove { dest, .. }
            | IrInstruction::CollectionPop { dest, .. }
            | IrInstruction::Settle { dest, .. }
            | IrInstruction::Transfer { dest, .. }
            | IrInstruction::Move { dest, .. }
            | IrInstruction::Tuple { dest, .. }
            | IrInstruction::EnumConstruct { dest, .. }
            | IrInstruction::EnumTag { dest, .. }
            | IrInstruction::EnumPayload { dest, .. }
            | IrInstruction::Binary { dest, .. }
            | IrInstruction::Call { dest: Some(dest), .. } => {
                if dest.ty == IrType::U128 {
                    out.insert(dest.id);
                }
            }
            IrInstruction::CollectionNew { dest, .. } => {
                if dest.ty == IrType::U128 {
                    out.insert(dest.id);
                }
            }
            IrInstruction::StoreVar { .. }
            | IrInstruction::Call { dest: None, .. }
            | IrInstruction::Consume { .. }
            | IrInstruction::Destroy { .. }
            | IrInstruction::CellMetadataEquality { .. }
            | IrInstruction::CollectionPush { .. }
            | IrInstruction::CollectionExtend { .. }
            | IrInstruction::CollectionClear { .. }
            | IrInstruction::CollectionReverse { .. }
            | IrInstruction::CollectionTruncate { .. }
            | IrInstruction::CollectionSwap { .. }
            | IrInstruction::CollectionInsert { .. }
            | IrInstruction::CollectionSet { .. } => {}
        }
    }

    pub(super) fn collect_u128_terminator_vars(&self, terminator: &IrTerminator, out: &mut BTreeSet<usize>) {
        if let IrTerminator::Return(Some(IrOperand::Var(var))) = terminator
            && var.ty == IrType::U128
        {
            out.insert(var.id);
        }
    }

    pub(super) fn record_operand(&self, operand: &IrOperand, max_var_id: &mut Option<usize>) {
        if let IrOperand::Var(var) = operand {
            self.record_var(var, max_var_id);
        }
    }

    pub(super) fn record_var(&self, var: &IrVar, max_var_id: &mut Option<usize>) {
        *max_var_id = Some(max_var_id.map(|current| current.max(var.id)).unwrap_or(var.id));
    }

    pub(super) fn const_as_u128(value: &IrConst) -> Option<u128> {
        match value {
            IrConst::U8(value) => Some((*value).into()),
            IrConst::U16(value) => Some((*value).into()),
            IrConst::U32(value) => Some((*value).into()),
            IrConst::U64(value) => Some((*value).into()),
            IrConst::U128(value) => Some(*value),
            _ => None,
        }
    }

    pub(super) fn expected_u128_source(&self, operand: &IrOperand) -> Option<ExpectedFixedByteSource> {
        match operand {
            IrOperand::Const(value) => {
                Self::const_as_u128(value).map(|value| ExpectedFixedByteSource::Const(value.to_le_bytes().to_vec()))
            }
            _ => self.expected_fixed_byte_source(operand, 16),
        }
    }

    pub(super) fn emit_store_byte_to_stack_offset(&mut self, src_reg: &str, offset: usize) {
        self.emit_stack_store_byte(src_reg, offset);
    }

    pub(super) fn emit_store_u128_const_to_stack_offset(&mut self, value: u128, offset: usize) {
        self.emit(format!("# cellscript abi: materialize u128 const at stack+{}", offset));
        for (index, byte) in value.to_le_bytes().iter().enumerate() {
            self.emit(format!("li t0, {}", byte));
            self.emit_store_byte_to_stack_offset("t0", offset + index);
        }
    }

    pub(super) fn emit_store_u128_pointer_for_var(&mut self, var_id: usize, offset: usize) {
        self.emit_sp_addi("t0", offset);
        self.emit_stack_store("t0", var_id * 8);
    }

    pub(super) fn emit_materialize_u128_operand_to_var(&mut self, dest: &IrVar, src: &IrOperand) -> bool {
        let Some(dest_offset) = self.u128_value_offsets.get(&dest.id).copied() else {
            self.emit("# cellscript abi: u128 destination has no 16-byte storage; fail closed");
            self.emit_fail(CellScriptRuntimeError::FixedByteComparisonUnresolved);
            return true;
        };
        if let IrOperand::Const(value) = src
            && let Some(value) = Self::const_as_u128(value)
        {
            self.emit_store_u128_const_to_stack_offset(value, dest_offset);
            self.emit_store_u128_pointer_for_var(dest.id, dest_offset);
            return true;
        }
        let Some(source) = self.expected_u128_source(src) else {
            self.emit("# cellscript abi: u128 source is not addressable; fail closed");
            self.emit_fail(CellScriptRuntimeError::FixedByteComparisonUnresolved);
            return true;
        };
        self.emit_prepare_fixed_byte_source(&source, 16, "u128 materialize");
        self.emit(format!("# cellscript abi: materialize u128 operand into var{}", dest.id));
        for byte_index in 0..16 {
            self.emit_fixed_byte_source_byte_to("t0", "t4", &source, byte_index);
            self.emit_store_byte_to_stack_offset("t0", dest_offset + byte_index);
        }
        self.emit_store_u128_pointer_for_var(dest.id, dest_offset);
        true
    }

    pub(super) fn emit_u64_le_from_fixed_byte_source(
        &mut self,
        dest_reg: &str,
        scratch_reg: &str,
        base_reg: &str,
        source: &ExpectedFixedByteSource,
        start: usize,
    ) {
        self.emit(format!("li {}, 0", dest_reg));
        for byte_offset in 0..8 {
            self.emit_fixed_byte_source_byte_to(scratch_reg, base_reg, source, start + byte_offset);
            if byte_offset != 0 {
                self.emit(format!("slli {}, {}, {}", scratch_reg, scratch_reg, byte_offset * 8));
            }
            self.emit(format!("or {}, {}, {}", dest_reg, dest_reg, scratch_reg));
        }
    }

    pub(super) fn emit_u128_operand_limbs(
        &mut self,
        low_reg: &str,
        high_reg: &str,
        scratch_reg: &str,
        base_reg: &str,
        operand: &IrOperand,
        context: &str,
    ) -> bool {
        let Some(source) = self.expected_u128_source(operand) else {
            self.emit(format!("# cellscript abi: {} u128 operand is not addressable; fail closed", context));
            self.emit_fail(CellScriptRuntimeError::FixedByteComparisonUnresolved);
            return false;
        };
        self.emit_prepare_fixed_byte_source(&source, 16, context);
        self.emit_u64_le_from_fixed_byte_source(low_reg, scratch_reg, base_reg, &source, 0);
        self.emit_u64_le_from_fixed_byte_source(high_reg, scratch_reg, base_reg, &source, 8);
        true
    }

    pub(super) fn operand_is_u128_like(&self, operand: &IrOperand) -> bool {
        match operand {
            IrOperand::Var(var) => var.ty == IrType::U128,
            IrOperand::Const(IrConst::U128(_)) => true,
            _ => false,
        }
    }

    pub(super) fn emit_store_const_bytes_to_stack(&mut self, bytes: &[u8], offset: usize) {
        for (index, byte) in bytes.iter().enumerate() {
            self.emit(format!("li t0, {}", byte));
            self.emit_stack_store_byte("t0", offset + index);
        }
    }
}
