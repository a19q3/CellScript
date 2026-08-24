use super::*;

impl CodeGenerator {
    pub(super) fn emit_entry_abi_marker(&mut self, name: &str) {
        self.assembly.push(format!("# cellscript entry abi: {} requires-explicit-parameter-abi", name));
    }

    pub(super) fn emit_entry_direct_wrapper(&mut self, target: &str) {
        self.emit_global(ENTRY_WITNESS_LABEL);
        self.emit_label(ENTRY_WITNESS_LABEL);
        self.emit(format!("# cellscript entry abi: {} tail-calls no-arg {}", ENTRY_WITNESS_LABEL, target));
        self.emit(format!("j {}", target));
    }

    pub(super) fn emit_entry_witness_wrapper(&mut self, target: &str, params: &[IrParam]) -> Result<()> {
        self.entry_frame_sizes.insert(ENTRY_WITNESS_LABEL.to_string(), ENTRY_WITNESS_FRAME_SIZE as u32);
        let callable_abi = self.callable_abis.get(target).cloned();
        let type_hash_param_indices = callable_abi.as_ref().map(|abi| abi.type_hash_param_indices.clone()).unwrap_or_default();
        let runtime_bound_param_indices = callable_abi.as_ref().map(|abi| abi.runtime_bound_param_indices.clone()).unwrap_or_default();
        let outgoing_stack_arg_bytes = align_stack_arg_bytes(entry_abi_arg_count(params, callable_abi.as_ref()).saturating_sub(8) * 8);
        let payload = entry_witness_payload_layout(params, &runtime_bound_param_indices, &self.enum_layouts);
        let payload_len = payload.iter().map(|arg| arg.width).sum::<usize>();
        let has_witness_payload = payload.iter().any(|arg| arg.width > 0 || arg.unsupported);
        let has_lock_args = params.iter().any(|param| param.source == ParamSource::LockArgs);
        let has_dynamic_payload = payload.iter().any(|arg| arg.schema_dynamic);
        let min_witness_len = ENTRY_WITNESS_HEADER_SIZE + payload_len;
        let loaded_label = self.fresh_label("entry_witness_loaded");
        let try_group_output_label = self.fresh_label("entry_witness_try_group_output");
        let buffer_ok_label = self.fresh_label("entry_witness_buffer_ok");
        let size_ok_label = self.fresh_label("entry_witness_size_ok");
        let fail_label = self.fresh_label("entry_witness_fail");
        let done_label = self.fresh_label("entry_witness_done");

        self.emit_global(ENTRY_WITNESS_LABEL);
        self.emit_label(ENTRY_WITNESS_LABEL);
        self.emit(format!(
            "# cellscript entry abi: {} loads GroupInput#0 witness args for {} and falls back to GroupOutput#0",
            ENTRY_WITNESS_LABEL, target
        ));
        self.emit("# cellscript entry abi: placement profile requires CSARGv1 inside WitnessArgs.input_type");
        self.emit_large_addi("sp", "sp", -(ENTRY_WITNESS_FRAME_SIZE as i64));
        self.emit_stack_store("ra", ENTRY_WITNESS_RA_OFFSET);
        if has_lock_args {
            self.emit_entry_load_script_args(&fail_label);
        }
        if has_witness_payload {
            self.emit_load_witness_syscall_to_offsets(
                "entry_args",
                CKB_SOURCE_GROUP_INPUT,
                0,
                ENTRY_WITNESS_SIZE_OFFSET,
                ENTRY_WITNESS_BUFFER_OFFSET,
                ENTRY_WITNESS_BUFFER_SIZE,
            );
            self.emit(format!("beqz a0, {}", loaded_label));
            self.emit(format!("j {}", try_group_output_label));
            self.emit_label(&try_group_output_label);
            self.emit_load_witness_syscall_to_offsets(
                "entry_args_fallback_group_output",
                CKB_SOURCE_GROUP_OUTPUT,
                0,
                ENTRY_WITNESS_SIZE_OFFSET,
                ENTRY_WITNESS_BUFFER_OFFSET,
                ENTRY_WITNESS_BUFFER_SIZE,
            );
            self.emit(format!("beqz a0, {}", loaded_label));
            self.emit(format!("j {}", fail_label));
            self.emit_label(&loaded_label);

            self.emit_stack_load("t0", ENTRY_WITNESS_SIZE_OFFSET);
            self.emit("# cellscript entry abi: reject witnesses larger than the local entry buffer");
            self.emit(format!("li t1, {}", ENTRY_WITNESS_BUFFER_SIZE + 1));
            self.emit("sltu t2, t0, t1");
            self.emit(format!("bnez t2, {}", buffer_ok_label));
            self.emit(format!("j {}", fail_label));
            self.emit_label(&buffer_ok_label);

            self.emit_entry_normalize_witness_args_input_type_v2(&fail_label);

            self.emit_stack_load("t0", ENTRY_WITNESS_SIZE_OFFSET);
            self.emit(format!("li t1, {}", min_witness_len));
            self.emit("sltu t2, t0, t1");
            self.emit(format!("beqz t2, {}", size_ok_label));
            self.emit(format!("j {}", fail_label));
            self.emit_label(&size_ok_label);

            for (index, byte) in ENTRY_WITNESS_MAGIC.iter().enumerate() {
                self.emit_stack_load_byte("t0", ENTRY_WITNESS_BUFFER_OFFSET + index);
                self.emit(format!("li t1, {}", byte));
                self.emit("sub t2, t0, t1");
                self.emit(format!("bnez t2, {}", fail_label));
            }

            if !has_dynamic_payload {
                let exact_size_label = self.fresh_label("entry_witness_exact_size_ok");
                self.emit("# cellscript entry abi: reject trailing witness payload bytes");
                self.emit_stack_load("t0", ENTRY_WITNESS_SIZE_OFFSET);
                self.emit(format!("li t1, {}", min_witness_len));
                self.emit("sub t2, t0, t1");
                self.emit(format!("beqz t2, {}", exact_size_label));
                self.emit(format!("j {}", fail_label));
                self.emit_label(&exact_size_label);
            }
        }

        if payload.iter().any(|arg| arg.unsupported) {
            self.emit("# cellscript entry abi: unsupported witness parameter shape; fail closed");
            self.emit(format!("j {}", fail_label));
        } else if has_dynamic_payload {
            let mut abi_index = 0usize;
            self.emit("# cellscript entry abi: witness payload contains schema-backed dynamic segments");
            self.emit_stack_load("t5", ENTRY_WITNESS_SIZE_OFFSET);
            self.emit(format!("li t6, {}", ENTRY_WITNESS_HEADER_SIZE));
            for (param_index, param) in params.iter().enumerate() {
                let param_is_runtime_bound =
                    runtime_bound_param_indices.contains(&param_index) || matches!(param.ty, IrType::Ref(_) | IrType::MutRef(_));
                if param.source == ParamSource::LockArgs {
                    self.emit_entry_lock_args_param(&mut abi_index, param, outgoing_stack_arg_bytes, &fail_label);
                } else if param_is_runtime_bound {
                    self.emit(format!("# cellscript entry abi: runtime-bound param {} is loaded from transaction cells", param.name));
                    self.emit_entry_abi_zero_arg(abi_index, outgoing_stack_arg_bytes);
                    self.emit_entry_abi_zero_arg(abi_index + 1, outgoing_stack_arg_bytes);
                    abi_index += 2;
                    if type_hash_param_indices.contains(&param_index) {
                        self.emit(format!(
                            "# cellscript entry abi: runtime-bound param {} TypeHash witness bytes unavailable; pass null ABI bytes",
                            param.name
                        ));
                        self.emit_entry_abi_zero_arg(abi_index, outgoing_stack_arg_bytes);
                        self.emit_entry_abi_zero_arg(abi_index + 1, outgoing_stack_arg_bytes);
                        abi_index += 2;
                    }
                } else if entry_witness_dynamic_schema_param(&param.ty) && self.payload_enum_width(&param.ty).is_none() {
                    let len_ok_label = self.fresh_label("entry_witness_schema_len_ok");
                    let bytes_ok_label = self.fresh_label("entry_witness_schema_bytes_ok");
                    self.emit(format!(
                        "# cellscript entry abi: schema param {} -> {}={} {}={} (length-prefixed witness bytes)",
                        param.name,
                        abi_arg_label(abi_index),
                        "ptr",
                        abi_arg_label(abi_index + 1),
                        "len"
                    ));
                    self.emit("addi t1, t6, 4");
                    self.emit("sltu t2, t5, t1");
                    self.emit(format!("beqz t2, {}", len_ok_label));
                    self.emit(format!("j {}", fail_label));
                    self.emit_label(&len_ok_label);
                    self.emit("add t0, sp, t6");
                    self.emit(format!("addi t0, t0, {}", ENTRY_WITNESS_BUFFER_OFFSET));
                    self.emit("li t4, 0");
                    for byte_index in 0..4 {
                        self.emit(format!("lbu t1, {}(t0)", byte_index));
                        if byte_index != 0 {
                            self.emit(format!("slli t1, t1, {}", byte_index * 8));
                        }
                        self.emit("or t4, t4, t1");
                    }
                    self.emit("addi t1, t6, 4");
                    self.emit("add t1, t1, t4");
                    self.emit("sltu t2, t5, t1");
                    self.emit(format!("beqz t2, {}", bytes_ok_label));
                    self.emit(format!("j {}", fail_label));
                    self.emit_label(&bytes_ok_label);
                    self.emit_entry_abi_pointer_from_dynamic_offset(abi_index, "t6", 4, "t0", outgoing_stack_arg_bytes);
                    self.emit_entry_abi_reg_arg(abi_index + 1, "t4", outgoing_stack_arg_bytes);
                    abi_index += 2;
                    self.emit("addi t6, t6, 4");
                    self.emit("add t6, t6, t4");
                    if type_hash_param_indices.contains(&param_index) {
                        self.emit(format!(
                            "# cellscript entry abi: schema param {} TypeHash witness bytes unavailable; pass null ABI bytes",
                            param.name
                        ));
                        self.emit_entry_abi_zero_arg(abi_index, outgoing_stack_arg_bytes);
                        self.emit_entry_abi_zero_arg(abi_index + 1, outgoing_stack_arg_bytes);
                        abi_index += 2;
                    }
                } else if let Some(width) = self
                    .payload_enum_width(&param.ty)
                    .or_else(|| fixed_byte_pointer_param_width(&param.ty).or_else(|| fixed_aggregate_pointer_param_width(&param.ty)))
                {
                    let bytes_ok_label = self.fresh_label("entry_witness_fixed_bytes_ok");
                    self.emit(format!(
                        "# cellscript entry abi: fixed-byte param {} pointer={} length={} size={}",
                        param.name,
                        abi_arg_label(abi_index),
                        abi_arg_label(abi_index + 1),
                        width
                    ));
                    self.emit(format!("addi t1, t6, {}", width));
                    self.emit("sltu t2, t5, t1");
                    self.emit(format!("beqz t2, {}", bytes_ok_label));
                    self.emit(format!("j {}", fail_label));
                    self.emit_label(&bytes_ok_label);
                    self.emit_entry_abi_pointer_from_dynamic_offset(abi_index, "t6", 0, "t0", outgoing_stack_arg_bytes);
                    self.emit_entry_abi_immediate_arg(abi_index + 1, width as u64, outgoing_stack_arg_bytes);
                    self.emit(format!("addi t6, t6, {}", width));
                    abi_index += 2;
                } else if let Some(width) = entry_witness_register_param_width(&param.ty) {
                    let bytes_ok_label = self.fresh_label("entry_witness_scalar_bytes_ok");
                    self.emit(format!(
                        "# cellscript entry abi: scalar param {} -> {} size={}",
                        param.name,
                        abi_arg_label(abi_index),
                        width
                    ));
                    self.emit(format!("addi t1, t6, {}", width));
                    self.emit("sltu t2, t5, t1");
                    self.emit(format!("beqz t2, {}", bytes_ok_label));
                    self.emit(format!("j {}", fail_label));
                    self.emit_label(&bytes_ok_label);
                    self.emit("add t0, sp, t6");
                    self.emit(format!("addi t0, t0, {}", ENTRY_WITNESS_BUFFER_OFFSET));
                    if abi_index < 8 {
                        self.emit_entry_witness_scalar_load_from_reg(
                            &format!("a{}", abi_index),
                            "t0",
                            "t1",
                            width,
                            param.ty == IrType::I32,
                        );
                    } else {
                        let caller_stack_offset = (abi_index - 8) * 8;
                        self.emit_entry_witness_scalar_load_from_reg("t3", "t0", "t1", width, param.ty == IrType::I32);
                        self.emit(format!(
                            "# cellscript entry abi: scalar param {} stored to caller stack +{}",
                            param.name, caller_stack_offset
                        ));
                        self.emit_entry_abi_reg_arg(abi_index, "t3", outgoing_stack_arg_bytes);
                    }
                    self.emit(format!("addi t6, t6, {}", width));
                    abi_index += 1;
                } else {
                    self.emit(format!("# cellscript entry abi: unsupported param {} shape; fail closed", param.name));
                    self.emit(format!("j {}", fail_label));
                }
            }
            let exact_size_label = self.fresh_label("entry_witness_exact_size_ok");
            self.emit("# cellscript entry abi: reject trailing witness payload bytes");
            self.emit_stack_load("t5", ENTRY_WITNESS_SIZE_OFFSET);
            self.emit("sub t2, t5, t6");
            self.emit(format!("beqz t2, {}", exact_size_label));
            self.emit(format!("j {}", fail_label));
            self.emit_label(&exact_size_label);
            if has_lock_args {
                self.emit_entry_lock_args_exact_size_check(&fail_label);
            }
            self.emit_entry_call_target(target, outgoing_stack_arg_bytes);
            self.emit(format!("j {}", done_label));
        } else {
            let mut abi_index = 0usize;
            let mut payload_cursor = 0usize;
            for (param_index, param) in params.iter().enumerate() {
                let param_is_runtime_bound =
                    runtime_bound_param_indices.contains(&param_index) || matches!(param.ty, IrType::Ref(_) | IrType::MutRef(_));
                if param.source == ParamSource::LockArgs {
                    self.emit_entry_lock_args_param(&mut abi_index, param, outgoing_stack_arg_bytes, &fail_label);
                } else if param_is_runtime_bound {
                    self.emit(format!("# cellscript entry abi: runtime-bound param {} is loaded from transaction cells", param.name));
                    self.emit_entry_abi_zero_arg(abi_index, outgoing_stack_arg_bytes);
                    self.emit_entry_abi_zero_arg(abi_index + 1, outgoing_stack_arg_bytes);
                    abi_index += 2;
                    if type_hash_param_indices.contains(&param_index) {
                        self.emit(format!(
                            "# cellscript entry abi: runtime-bound param {} TypeHash witness bytes unavailable; pass null ABI bytes",
                            param.name
                        ));
                        self.emit_entry_abi_zero_arg(abi_index, outgoing_stack_arg_bytes);
                        self.emit_entry_abi_zero_arg(abi_index + 1, outgoing_stack_arg_bytes);
                        abi_index += 2;
                    }
                } else if entry_witness_dynamic_schema_param(&param.ty) && self.payload_enum_width(&param.ty).is_none() {
                    self.emit(format!("# cellscript entry abi: schema param {} is runtime-loaded; pass null ABI bytes", param.name));
                    self.emit_entry_abi_zero_arg(abi_index, outgoing_stack_arg_bytes);
                    self.emit_entry_abi_zero_arg(abi_index + 1, outgoing_stack_arg_bytes);
                    abi_index += 2;
                    if type_hash_param_indices.contains(&param_index) {
                        self.emit(format!(
                            "# cellscript entry abi: schema param {} TypeHash witness bytes unavailable; pass null ABI bytes",
                            param.name
                        ));
                        self.emit_entry_abi_zero_arg(abi_index, outgoing_stack_arg_bytes);
                        self.emit_entry_abi_zero_arg(abi_index + 1, outgoing_stack_arg_bytes);
                        abi_index += 2;
                    }
                } else if let Some(width) = self
                    .payload_enum_width(&param.ty)
                    .or_else(|| fixed_byte_pointer_param_width(&param.ty).or_else(|| fixed_aggregate_pointer_param_width(&param.ty)))
                {
                    self.emit(format!(
                        "# cellscript entry abi: fixed-byte param {} pointer={} length={} size={}",
                        param.name,
                        abi_arg_label(abi_index),
                        abi_arg_label(abi_index + 1),
                        width
                    ));
                    self.emit_entry_abi_pointer_arg(
                        abi_index,
                        ENTRY_WITNESS_BUFFER_OFFSET + ENTRY_WITNESS_HEADER_SIZE + payload_cursor,
                        outgoing_stack_arg_bytes,
                    );
                    self.emit_entry_abi_immediate_arg(abi_index + 1, width as u64, outgoing_stack_arg_bytes);
                    payload_cursor += width;
                    abi_index += 2;
                } else if let Some(width) = entry_witness_register_param_width(&param.ty) {
                    self.emit(format!(
                        "# cellscript entry abi: scalar param {} -> {} size={}",
                        param.name,
                        abi_arg_label(abi_index),
                        width
                    ));
                    let stack_offset = ENTRY_WITNESS_BUFFER_OFFSET + ENTRY_WITNESS_HEADER_SIZE + payload_cursor;
                    if abi_index < 8 {
                        self.emit_entry_witness_scalar_load(&format!("a{}", abi_index), stack_offset, width, param.ty == IrType::I32);
                    } else {
                        let caller_stack_offset = (abi_index - 8) * 8;
                        self.emit_entry_witness_scalar_load("t3", stack_offset, width, param.ty == IrType::I32);
                        self.emit(format!(
                            "# cellscript entry abi: scalar param {} stored to caller stack +{}",
                            param.name, caller_stack_offset
                        ));
                        self.emit_entry_abi_reg_arg(abi_index, "t3", outgoing_stack_arg_bytes);
                    }
                    payload_cursor += width;
                    abi_index += 1;
                } else {
                    self.emit(format!("# cellscript entry abi: unsupported param {} shape; fail closed", param.name));
                    self.emit(format!("j {}", fail_label));
                }
            }
            if has_lock_args {
                self.emit_entry_lock_args_exact_size_check(&fail_label);
            }
            self.emit_entry_call_target(target, outgoing_stack_arg_bytes);
            self.emit(format!("j {}", done_label));
        }

        self.emit_label(&fail_label);
        self.emit_runtime_error_comment(CellScriptRuntimeError::EntryWitnessAbiInvalid);
        self.emit(format!("li a0, {}", CellScriptRuntimeError::EntryWitnessAbiInvalid.code()));
        self.emit_label(&done_label);
        self.emit_stack_load("ra", ENTRY_WITNESS_RA_OFFSET);
        self.emit_large_addi("sp", "sp", ENTRY_WITNESS_FRAME_SIZE as i64);
        self.emit("ret");
        Ok(())
    }

    /// Normalize the selected entry placement ABI into the payload buffer
    /// shape consumed by the positional decoder.
    ///
    /// The wrapper requires a canonical CKB `WitnessArgs` from the current
    /// script group and copies its `input_type` Bytes payload to the start of
    /// the local buffer. A raw `CSARGv1\0` witness is not a valid alias.
    pub(super) fn emit_entry_normalize_witness_args_input_type_v2(&mut self, fail_label: &str) {
        let validate_loop_label = self.fresh_label("entry_witness_v2_validate_loop");
        let field_end_ready_label = self.fresh_label("entry_witness_v2_field_end_ready");
        let field_done_label = self.fresh_label("entry_witness_v2_field_done");
        let copy_loop_label = self.fresh_label("entry_witness_v2_copy_loop");
        let copy_done_label = self.fresh_label("entry_witness_v2_copy_done");

        self.emit("# cellscript entry placement profile: validate the exact three-field WitnessArgs table");
        self.emit_stack_load("t0", ENTRY_WITNESS_SIZE_OFFSET);
        self.emit("li t1, 16");
        self.emit(format!("bltu t0, t1, {}", fail_label));
        self.emit_sp_addi("t3", ENTRY_WITNESS_BUFFER_OFFSET);

        // The table header and local buffer are eight-byte aligned, so load its
        // four u32 words in two pairs. Keep variable-offset Bytes lengths below
        // on byte loads because Molecule payload offsets need not be aligned.
        self.emit("ld a4, 0(t3)");
        self.emit("slli t1, a4, 32");
        self.emit("srli t1, t1, 32");
        self.emit(format!("bne t1, t0, {}", fail_label));
        self.emit("srli t4, a4, 32");
        self.emit("li t1, 16");
        self.emit(format!("bne t4, t1, {}", fail_label));
        self.emit("ld a4, 8(t3)");
        self.emit("slli t5, a4, 32");
        self.emit("srli t5, t5, 32");
        self.emit(format!("bltu t5, t4, {}", fail_label));
        self.emit("srli t6, a4, 32");
        self.emit(format!("bltu t6, t5, {}", fail_label));
        self.emit(format!("bltu t0, t6, {}", fail_label));

        // Validate lock, input_type, and output_type through one compact loop.
        // a5 is the field index and t4 the current start. The three ends are
        // the preserved input_type offset, output_type offset, and total_size.
        self.emit("li t4, 16");
        self.emit("li a5, 0");
        self.emit_label(&validate_loop_label);
        self.emit("addi a6, t5, 0");
        self.emit(format!("beqz a5, {}", field_end_ready_label));
        self.emit("addi a6, t6, 0");
        self.emit("li a0, 1");
        self.emit(format!("beq a5, a0, {}", field_end_ready_label));
        self.emit("addi a6, t0, 0");
        self.emit_label(&field_end_ready_label);
        self.emit("sub a1, a6, t4");
        self.emit(format!("beqz a1, {}", field_done_label));
        self.emit("li a0, 4");
        self.emit(format!("bltu a1, a0, {}", fail_label));
        self.emit("add a2, t3, t4");
        self.emit_u32_le_from_base_to("t1", "a2", 0, "t2");
        self.emit("addi a1, a1, -4");
        self.emit(format!("bne t1, a1, {}", fail_label));
        self.emit_label(&field_done_label);
        self.emit("addi t4, a6, 0");
        self.emit("addi a5, a5, 1");
        self.emit("li a0, 3");
        self.emit(format!("bltu a5, a0, {}", validate_loop_label));

        // input_type is mandatory for v2, while lock and output_type remain
        // optional. t5 and t6 still hold its start and end offsets.
        self.emit("sub t1, t6, t5");
        self.emit(format!("beqz t1, {}", fail_label));
        self.emit("addi t1, t1, -4");
        self.emit("add t4, t3, t5");

        self.emit("# cellscript entry placement v2: copy input_type payload over the table envelope");
        self.emit("addi t4, t4, 4");
        self.emit_sp_addi("t5", ENTRY_WITNESS_BUFFER_OFFSET);
        self.emit("li t2, 0");
        self.emit_label(&copy_loop_label);
        self.emit("sltu t6, t2, t1");
        self.emit(format!("beqz t6, {}", copy_done_label));
        self.emit("add t3, t4, t2");
        self.emit("lbu t6, 0(t3)");
        self.emit("add t3, t5, t2");
        self.emit("sb t6, 0(t3)");
        self.emit("addi t2, t2, 1");
        self.emit(format!("j {}", copy_loop_label));
        self.emit_label(&copy_done_label);
        self.emit_stack_store("t1", ENTRY_WITNESS_SIZE_OFFSET);
    }

    pub(super) fn emit_entry_call_target(&mut self, target: &str, outgoing_stack_arg_bytes: usize) {
        if outgoing_stack_arg_bytes > 0 {
            self.emit(format!("# cellscript entry abi: reserve {} bytes for outgoing stack call arguments", outgoing_stack_arg_bytes));
            self.emit_large_addi("sp", "sp", -(outgoing_stack_arg_bytes as i64));
        }
        self.emit(format!("call {}", target));
        if outgoing_stack_arg_bytes > 0 {
            self.emit_large_addi("sp", "sp", outgoing_stack_arg_bytes as i64);
        }
    }

    pub(super) fn emit_entry_abi_zero_arg(&mut self, abi_index: usize, outgoing_stack_arg_bytes: usize) {
        self.emit_entry_abi_immediate_arg(abi_index, 0, outgoing_stack_arg_bytes);
    }

    pub(super) fn emit_entry_abi_reg_arg(&mut self, abi_index: usize, source_reg: &str, outgoing_stack_arg_bytes: usize) {
        if abi_index < 8 {
            self.emit(format!("addi a{}, {}, 0", abi_index, source_reg));
        } else {
            self.emit_entry_outgoing_stack_arg_store(source_reg, abi_index, outgoing_stack_arg_bytes);
        }
    }

    pub(super) fn emit_entry_abi_immediate_arg(&mut self, abi_index: usize, value: u64, outgoing_stack_arg_bytes: usize) {
        if abi_index < 8 {
            self.emit(format!("li a{}, {}", abi_index, value));
        } else {
            self.emit(format!("# cellscript entry abi: stack arg{} <- {}", abi_index, value));
            self.emit(format!("li t0, {}", value));
            self.emit_entry_outgoing_stack_arg_store("t0", abi_index, outgoing_stack_arg_bytes);
        }
    }

    pub(super) fn emit_entry_abi_pointer_arg(&mut self, abi_index: usize, stack_offset: usize, outgoing_stack_arg_bytes: usize) {
        if abi_index < 8 {
            self.emit_sp_addi(&format!("a{}", abi_index), stack_offset);
        } else {
            self.emit(format!("# cellscript entry abi: stack arg{} <- sp+{}", abi_index, stack_offset));
            self.emit_sp_addi("t0", stack_offset);
            self.emit_entry_outgoing_stack_arg_store("t0", abi_index, outgoing_stack_arg_bytes);
        }
    }

    pub(super) fn emit_entry_abi_pointer_from_dynamic_offset(
        &mut self,
        abi_index: usize,
        offset_reg: &str,
        extra_offset: usize,
        temp_reg: &str,
        outgoing_stack_arg_bytes: usize,
    ) {
        self.emit(format!("add {}, sp, {}", temp_reg, offset_reg));
        if ENTRY_WITNESS_BUFFER_OFFSET + extra_offset != 0 {
            self.emit(format!("addi {}, {}, {}", temp_reg, temp_reg, ENTRY_WITNESS_BUFFER_OFFSET + extra_offset));
        }
        self.emit_entry_abi_reg_arg(abi_index, temp_reg, outgoing_stack_arg_bytes);
    }

    pub(super) fn emit_entry_outgoing_stack_arg_store(&mut self, register: &str, abi_index: usize, outgoing_stack_arg_bytes: usize) {
        let stack_slot_offset = (abi_index - 8) * 8;
        let offset = i64::try_from(stack_slot_offset).expect("entry call stack slot should fit in i64")
            - i64::try_from(outgoing_stack_arg_bytes).expect("entry call stack argument area should fit in i64");
        self.emit(format!(
            "# cellscript entry abi: stage stack arg{} at pre-call sp{}{}",
            abi_index,
            if offset < 0 { "" } else { "+" },
            offset
        ));
        self.emit_sp_store_signed(register, offset);
    }

    pub(super) fn emit_entry_witness_scalar_load(&mut self, dest_reg: &str, stack_offset: usize, width: usize, signed_i32: bool) {
        self.emit(format!("li {}, 0", dest_reg));
        for byte_index in 0..width {
            self.emit_stack_load_byte("t0", stack_offset + byte_index);
            if byte_index != 0 {
                self.emit(format!("slli t0, t0, {}", byte_index * 8));
            }
            self.emit(format!("or {}, {}, t0", dest_reg, dest_reg));
        }
        if signed_i32 {
            self.emit_sign_extend_i32(dest_reg);
        }
    }

    pub(super) fn emit_entry_witness_scalar_load_from_reg(
        &mut self,
        dest_reg: &str,
        base_reg: &str,
        byte_reg: &str,
        width: usize,
        signed_i32: bool,
    ) {
        debug_assert_ne!(dest_reg, base_reg, "entry scalar decoder destination must not alias its base");
        debug_assert_ne!(byte_reg, base_reg, "entry scalar decoder scratch must not alias its base");
        debug_assert_ne!(byte_reg, dest_reg, "entry scalar decoder scratch must not alias its destination");
        self.emit(format!("li {}, 0", dest_reg));
        for byte_index in 0..width {
            self.emit(format!("lbu {}, {}({})", byte_reg, byte_index, base_reg));
            if byte_index != 0 {
                self.emit(format!("slli {}, {}, {}", byte_reg, byte_reg, byte_index * 8));
            }
            self.emit(format!("or {}, {}, {}", dest_reg, dest_reg, byte_reg));
        }
        if signed_i32 {
            self.emit_sign_extend_i32(dest_reg);
        }
    }

    pub(super) fn emit_entry_load_u32_from_stack(&mut self, dest_reg: &str, stack_offset: usize) {
        self.emit(format!("li {}, 0", dest_reg));
        for byte_index in 0..4 {
            self.emit_stack_load_byte("t0", stack_offset + byte_index);
            if byte_index != 0 {
                self.emit(format!("slli t0, t0, {}", byte_index * 8));
            }
            self.emit(format!("or {}, {}, t0", dest_reg, dest_reg));
        }
    }

    pub(super) fn emit_entry_load_u32_from_reg(&mut self, dest_reg: &str, base_reg: &str, byte_reg: &str) {
        debug_assert_ne!(dest_reg, base_reg, "entry u32 decoder destination must not alias its base");
        debug_assert_ne!(byte_reg, base_reg, "entry u32 decoder scratch must not alias its base");
        debug_assert_ne!(byte_reg, dest_reg, "entry u32 decoder scratch must not alias its destination");
        self.emit(format!("li {}, 0", dest_reg));
        for byte_index in 0..4 {
            self.emit(format!("lbu {}, {}({})", byte_reg, byte_index, base_reg));
            if byte_index != 0 {
                self.emit(format!("slli {}, {}, {}", byte_reg, byte_reg, byte_index * 8));
            }
            self.emit(format!("or {}, {}, {}", dest_reg, dest_reg, byte_reg));
        }
    }

    pub(super) fn emit_entry_load_script_args(&mut self, fail_label: &str) {
        let loaded_label = self.fresh_label("entry_script_loaded");
        let buffer_ok_label = self.fresh_label("entry_script_buffer_ok");
        let total_ok_label = self.fresh_label("entry_script_total_ok");
        let table_header_ok_label = self.fresh_label("entry_script_table_header_ok");
        let args_offset_min_ok_label = self.fresh_label("entry_script_args_offset_min_ok");
        let args_offset_ok_label = self.fresh_label("entry_script_args_offset_ok");
        let args_span_ok_label = self.fresh_label("entry_script_args_span_ok");

        self.emit("# cellscript entry abi: lock_args parameters are decoded from the executing Script.args bytes");
        self.emit_load_script_syscall_to_offsets(
            "entry_lock_args",
            ENTRY_SCRIPT_SIZE_OFFSET,
            ENTRY_SCRIPT_BUFFER_OFFSET,
            ENTRY_SCRIPT_BUFFER_SIZE,
        );
        self.emit(format!("beqz a0, {}", loaded_label));
        self.emit(format!("j {}", fail_label));
        self.emit_label(&loaded_label);

        self.emit_stack_load("t0", ENTRY_SCRIPT_SIZE_OFFSET);
        self.emit(format!("li t1, {}", ENTRY_SCRIPT_BUFFER_SIZE + 1));
        self.emit("sltu t2, t0, t1");
        self.emit(format!("bnez t2, {}", buffer_ok_label));
        self.emit(format!("j {}", fail_label));
        self.emit_label(&buffer_ok_label);

        self.emit_entry_load_u32_from_stack("t3", ENTRY_SCRIPT_BUFFER_OFFSET);
        self.emit_stack_load("t0", ENTRY_SCRIPT_SIZE_OFFSET);
        self.emit("sub t2, t0, t3");
        self.emit(format!("beqz t2, {}", total_ok_label));
        self.emit(format!("j {}", fail_label));
        self.emit_label(&total_ok_label);

        self.emit("li t1, 16");
        self.emit("sltu t2, t3, t1");
        self.emit(format!("beqz t2, {}", table_header_ok_label));
        self.emit(format!("j {}", fail_label));
        self.emit_label(&table_header_ok_label);

        self.emit_entry_load_u32_from_stack("t4", ENTRY_SCRIPT_BUFFER_OFFSET + 12);
        self.emit("li t1, 16");
        self.emit("sltu t2, t4, t1");
        self.emit(format!("beqz t2, {}", args_offset_min_ok_label));
        self.emit(format!("j {}", fail_label));
        self.emit_label(&args_offset_min_ok_label);
        self.emit("addi t1, t4, 4");
        self.emit("sltu t2, t3, t1");
        self.emit(format!("beqz t2, {}", args_offset_ok_label));
        self.emit(format!("j {}", fail_label));
        self.emit_label(&args_offset_ok_label);

        self.emit_sp_addi("t0", ENTRY_SCRIPT_BUFFER_OFFSET);
        self.emit("add t0, t0, t4");
        self.emit_entry_load_u32_from_reg("t5", "t0", "t1");
        self.emit("addi t6, t4, 4");
        self.emit("add t1, t6, t5");
        self.emit("sltu t2, t3, t1");
        self.emit(format!("beqz t2, {}", args_span_ok_label));
        self.emit(format!("j {}", fail_label));
        self.emit_label(&args_span_ok_label);
        self.emit_stack_store_with_avoid("t6", ENTRY_SCRIPT_ARGS_START_OFFSET, &["t5"]);
        self.emit_stack_store("t5", ENTRY_SCRIPT_ARGS_LEN_OFFSET);
        self.emit("li t0, 0");
        self.emit_stack_store("t0", ENTRY_SCRIPT_ARGS_CURSOR_OFFSET);
    }

    pub(super) fn emit_entry_lock_args_param(
        &mut self,
        abi_index: &mut usize,
        param: &IrParam,
        outgoing_stack_arg_bytes: usize,
        fail_label: &str,
    ) {
        let fixed_byte_width = self
            .payload_enum_width(&param.ty)
            .or_else(|| fixed_byte_pointer_param_width(&param.ty).or_else(|| fixed_aggregate_pointer_param_width(&param.ty)));
        let scalar_width = entry_witness_register_param_width(&param.ty);
        let Some(width) = fixed_byte_width.or(scalar_width) else {
            self.emit(format!("# cellscript entry abi: unsupported lock_args param {} shape; fail closed", param.name));
            self.emit(format!("j {}", fail_label));
            return;
        };
        let bytes_ok_label = self.fresh_label("entry_lock_args_bytes_ok");
        self.emit(format!("# cellscript entry abi: lock_args param {} consumes {} script arg byte(s)", param.name, width));
        let witness_cursor_live = ["t5", "t6"];
        let witness_and_script_cursor_live = ["t3", "t5", "t6"];
        self.emit_stack_load_with_avoid("t3", ENTRY_SCRIPT_ARGS_CURSOR_OFFSET, &witness_cursor_live);
        self.emit_stack_load_with_avoid("t4", ENTRY_SCRIPT_ARGS_LEN_OFFSET, &witness_and_script_cursor_live);
        self.emit(format!("addi t1, t3, {}", width));
        self.emit("sltu t2, t4, t1");
        self.emit(format!("beqz t2, {}", bytes_ok_label));
        self.emit(format!("j {}", fail_label));
        self.emit_label(&bytes_ok_label);
        self.emit_stack_load_with_avoid("t4", ENTRY_SCRIPT_ARGS_START_OFFSET, &witness_and_script_cursor_live);
        self.emit("add t4, t4, t3");
        self.emit_sp_addi("t0", ENTRY_SCRIPT_BUFFER_OFFSET);
        self.emit("add t0, t0, t4");

        if fixed_byte_width.is_some() {
            self.emit_entry_abi_reg_arg(*abi_index, "t0", outgoing_stack_arg_bytes);
            self.emit_entry_abi_immediate_arg(*abi_index + 1, width as u64, outgoing_stack_arg_bytes);
            *abi_index += 2;
        } else if *abi_index < 8 {
            self.emit_entry_witness_scalar_load_from_reg(&format!("a{}", *abi_index), "t0", "t1", width, param.ty == IrType::I32);
            *abi_index += 1;
        } else {
            self.emit_entry_witness_scalar_load_from_reg("t4", "t0", "t1", width, param.ty == IrType::I32);
            self.emit_entry_abi_reg_arg(*abi_index, "t4", outgoing_stack_arg_bytes);
            *abi_index += 1;
        }

        self.emit_stack_load_with_avoid("t3", ENTRY_SCRIPT_ARGS_CURSOR_OFFSET, &witness_cursor_live);
        self.emit(format!("addi t3, t3, {}", width));
        self.emit_stack_store_with_avoid("t3", ENTRY_SCRIPT_ARGS_CURSOR_OFFSET, &witness_cursor_live);
    }

    pub(super) fn emit_entry_lock_args_exact_size_check(&mut self, fail_label: &str) {
        let exact_label = self.fresh_label("entry_lock_args_exact_size_ok");
        self.emit("# cellscript entry abi: reject trailing Script.args bytes after typed lock_args");
        self.emit_stack_load("t0", ENTRY_SCRIPT_ARGS_CURSOR_OFFSET);
        self.emit_stack_load("t1", ENTRY_SCRIPT_ARGS_LEN_OFFSET);
        self.emit("sub t2, t1, t0");
        self.emit(format!("beqz t2, {}", exact_label));
        self.emit(format!("j {}", fail_label));
        self.emit_label(&exact_label);
    }
}
