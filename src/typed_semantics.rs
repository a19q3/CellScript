//! Compiler emission of the versioned typed-semantic record consumed by the
//! standalone artifact checker. The checker owns validation; this module only
//! translates checked IR into the shared, parser-free schema.

use crate::ir::{self, IrInstruction, IrOperand, IrTerminator, IrType, IrVar};
use crate::CompileMetadata;
use cellscript_artifact_checker::{
    canonical_hash, TypedSemanticBlock, TypedSemanticBorrow, TypedSemanticCall, TypedSemanticConstant, TypedSemanticCreatePattern,
    TypedSemanticEntry, TypedSemanticField, TypedSemanticInstantiation, TypedSemanticLocal, TypedSemanticOperand,
    TypedSemanticOperation, TypedSemanticOperationDetail, TypedSemanticOwnership, TypedSemanticParam, TypedSemanticRecord,
    TypedSemanticRuntimeError, TypedSemanticType, TypedSemanticVariant, TypedSemanticVariantField, TYPED_SEMANTICS_SCHEMA,
    TYPED_SEMANTICS_VERSION,
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn build(module: &ir::IrModule, metadata: &CompileMetadata) -> TypedSemanticRecord {
    let mut types = module
        .external_type_defs
        .iter()
        .chain(module.items.iter().filter_map(|item| match item {
            ir::IrItem::TypeDef(definition) => Some(definition),
            _ => None,
        }))
        .map(|definition| {
            let mut fields = definition
                .fields
                .iter()
                .map(|field| TypedSemanticField {
                    name: field.name.clone(),
                    ty: render_type(&field.ty),
                    offset: u32::try_from(field.offset).unwrap_or(u32::MAX),
                    width_bytes: field.fixed_size.and_then(|width| u32::try_from(width).ok()),
                })
                .collect::<Vec<_>>();
            fields.sort_by(|left, right| left.offset.cmp(&right.offset).then(left.name.cmp(&right.name)));
            let encoded_size = definition
                .fields
                .iter()
                .try_fold(0usize, |end, field| Some(end.max(field.offset.checked_add(field.fixed_size?)?)))
                .and_then(|width| u32::try_from(width).ok());
            let kind = match definition.kind {
                ir::IrTypeKind::Resource => "resource",
                ir::IrTypeKind::Shared => "shared",
                ir::IrTypeKind::Receipt => "receipt",
                ir::IrTypeKind::Struct => "struct",
            };
            let mut capabilities =
                definition.capabilities.iter().map(|capability| capability.as_str().to_string()).collect::<Vec<_>>();
            capabilities.sort();
            capabilities.dedup();
            let identity_policy = identity_policy_label(&definition.identity);
            let tag_width_bytes = None;
            let variants = Vec::<TypedSemanticVariant>::new();
            let layout_hash = canonical_hash(
                "cellscript-typed-layout-v2",
                &(kind, encoded_size, &fields, tag_width_bytes, &variants, &capabilities, &identity_policy),
            )
            .expect("typed layout record is serializable");
            TypedSemanticType {
                name: definition.name.clone(),
                kind: kind.to_string(),
                encoded_size,
                fields,
                tag_width_bytes,
                variants,
                capabilities,
                identity_policy,
                layout_hash,
            }
        })
        .collect::<Vec<_>>();
    for layout in module.enum_layouts.values() {
        let mut variants = layout
            .variants
            .iter()
            .map(|variant| TypedSemanticVariant {
                name: variant.name.clone(),
                tag: u32::from(variant.tag),
                payload_width_bytes: u32::try_from(variant.payload_width).unwrap_or(u32::MAX),
                fields: variant
                    .fields
                    .iter()
                    .map(|field| TypedSemanticVariantField {
                        index: u32::try_from(field.index).unwrap_or(u32::MAX),
                        ty: render_type(&field.ty),
                        offset: u32::try_from(field.offset).unwrap_or(u32::MAX),
                        width_bytes: u32::try_from(field.width).unwrap_or(u32::MAX),
                        linear: field.linear,
                    })
                    .collect(),
            })
            .collect::<Vec<_>>();
        variants.sort_by(|left, right| left.tag.cmp(&right.tag).then(left.name.cmp(&right.name)));
        for variant in &mut variants {
            variant.fields.sort_by_key(|field| field.index);
        }
        let encoded_size = u32::try_from(layout.encoded_size).ok();
        let fields = Vec::<TypedSemanticField>::new();
        let tag_width_bytes = u32::try_from(layout.tag_width).ok();
        let capabilities = Vec::<String>::new();
        let identity_policy = "none".to_string();
        let layout_hash = canonical_hash(
            "cellscript-typed-layout-v2",
            &("enum", encoded_size, &fields, tag_width_bytes, &variants, &capabilities, &identity_policy),
        )
        .expect("typed enum layout record is serializable");
        types.push(TypedSemanticType {
            name: layout.name.clone(),
            kind: "enum".to_string(),
            encoded_size,
            fields,
            tag_width_bytes,
            variants,
            capabilities,
            identity_policy,
            layout_hash,
        });
    }

    let signatures = callable_signatures(module);
    let mut entries = module
        .items
        .iter()
        .filter_map(|item| match item {
            ir::IrItem::Action(action) => Some(build_entry(
                "action",
                &action.name,
                &action.params,
                action.return_type.as_ref(),
                &format!("{:?}", action.effect_class),
                &action.body,
                &signatures,
                proof_ids(metadata, "action", &action.name),
            )),
            ir::IrItem::PureFn(function) => Some(build_entry(
                "helper",
                &function.name,
                &function.params,
                function.return_type.as_ref(),
                &format!("{:?}", function.effect_class),
                &function.body,
                &signatures,
                proof_ids(metadata, "helper", &function.name),
            )),
            ir::IrItem::Lock(lock) => Some(build_entry(
                "lock",
                &lock.name,
                &lock.params,
                Some(&IrType::Bool),
                "lock-predicate",
                &lock.body,
                &signatures,
                proof_ids(metadata, "lock", &lock.name),
            )),
            ir::IrItem::TypeDef(_) | ir::IrItem::Invariant(_) => None,
        })
        .collect::<Vec<_>>();
    let instantiations = metadata
        .generic_instantiations
        .iter()
        .map(|item| TypedSemanticInstantiation {
            kind: item.kind.clone(),
            module: item.module.clone(),
            template: item.template.clone(),
            concrete_name: item.concrete_name.clone(),
            identity: item.identity.clone(),
            type_arguments: item.type_arguments.clone(),
            value_ability_registry_version: item.value_ability_registry_version,
            constraints_verified: item.constraints_verified,
            fixed_layout_required: item.fixed_layout_required,
            cell_backed_layout_rejected: item.cell_backed_layout_rejected,
            identity_includes_phantom_arguments: item.identity_includes_phantom_arguments,
        })
        .collect::<Vec<_>>();
    let mut record = TypedSemanticRecord {
        schema: TYPED_SEMANTICS_SCHEMA.to_string(),
        version: TYPED_SEMANTICS_VERSION,
        module: module.name.clone(),
        interface_hash: String::new(),
        types: {
            types.sort_by(|left, right| left.name.cmp(&right.name));
            types.dedup_by(|left, right| left.name == right.name);
            types
        },
        entries: {
            entries.sort_by(|left, right| left.id.cmp(&right.id));
            entries
        },
        instantiations,
    };
    record.canonicalize();
    record
}

#[derive(Clone)]
struct CallableSignature {
    params: Vec<String>,
    return_type: String,
    effect: String,
    contract: String,
}

fn callable_signatures(module: &ir::IrModule) -> BTreeMap<String, CallableSignature> {
    let mut signatures = BTreeMap::new();
    for item in &module.items {
        match item {
            ir::IrItem::Action(action) => {
                signatures.insert(
                    action.name.clone(),
                    CallableSignature {
                        params: action.params.iter().map(|param| render_type(&param.ty)).collect(),
                        return_type: action.return_type.as_ref().map(render_type).unwrap_or_else(|| "unit".to_string()),
                        effect: format!("{:?}", action.effect_class),
                        contract: "typed-local".to_string(),
                    },
                );
            }
            ir::IrItem::PureFn(function) => {
                signatures.insert(
                    function.name.clone(),
                    CallableSignature {
                        params: function.params.iter().map(|param| render_type(&param.ty)).collect(),
                        return_type: function.return_type.as_ref().map(render_type).unwrap_or_else(|| "unit".to_string()),
                        effect: format!("{:?}", function.effect_class),
                        contract: "typed-local".to_string(),
                    },
                );
            }
            ir::IrItem::Lock(lock) => {
                signatures.insert(
                    lock.name.clone(),
                    CallableSignature {
                        params: lock.params.iter().map(|param| render_type(&param.ty)).collect(),
                        return_type: "bool".to_string(),
                        effect: "lock-predicate".to_string(),
                        contract: "typed-local".to_string(),
                    },
                );
            }
            ir::IrItem::TypeDef(_) | ir::IrItem::Invariant(_) => {}
        }
    }
    signatures
}

fn build_entry(
    kind: &str,
    name: &str,
    params: &[ir::IrParam],
    return_type: Option<&IrType>,
    effect: &str,
    body: &ir::IrBody,
    signatures: &BTreeMap<String, CallableSignature>,
    obligations: Vec<String>,
) -> TypedSemanticEntry {
    let mut locals = LocalTable::default();
    for param in params {
        insert_var(&mut locals, &param.binding);
    }
    let mut blocks = body
        .blocks
        .iter()
        .map(|block| {
            let mut operations =
                block.instructions.iter().map(|instruction| operation(instruction, &mut locals, signatures)).collect::<Vec<_>>();
            if let IrTerminator::Return(operand) = &block.terminator {
                operations.push(TypedSemanticOperation {
                    index: 0,
                    opcode: "return".to_string(),
                    destinations: Vec::new(),
                    operands: operand.iter().map(|operand| typed_operand(operand, &mut locals)).collect(),
                    detail: TypedSemanticOperationDetail::None,
                    call: None,
                });
            }
            if let IrTerminator::Branch { cond, .. } = &block.terminator {
                operations.push(TypedSemanticOperation {
                    index: 0,
                    opcode: "branch-condition".to_string(),
                    destinations: Vec::new(),
                    operands: vec![typed_operand(cond, &mut locals)],
                    detail: TypedSemanticOperationDetail::None,
                    call: None,
                });
            }
            for (index, operation) in operations.iter_mut().enumerate() {
                operation.index = u32::try_from(index).unwrap_or(u32::MAX);
            }
            let (terminator, successors) = match &block.terminator {
                IrTerminator::Return(_) => ("return", Vec::new()),
                IrTerminator::Jump(target) => ("jump", vec![u32::try_from(target.0).unwrap_or(u32::MAX)]),
                IrTerminator::Branch { then_block, else_block, .. } => {
                    ("branch", vec![u32::try_from(then_block.0).unwrap_or(u32::MAX), u32::try_from(else_block.0).unwrap_or(u32::MAX)])
                }
            };
            TypedSemanticBlock {
                id: u32::try_from(block.id.0).unwrap_or(u32::MAX),
                operations,
                successors,
                terminator: terminator.to_string(),
                runtime_error: block
                    .runtime_error
                    .map(|error| TypedSemanticRuntimeError { code: error.code(), name: error.name().to_string() }),
            }
        })
        .collect::<Vec<_>>();
    blocks.sort_by_key(|block| block.id);
    let param_types = params.iter().map(|param| (param.name.as_str(), render_type(&param.ty))).collect::<BTreeMap<_, _>>();
    let mut ownership = Vec::new();
    for pattern in &body.consume_set {
        let operation = pattern.operation.as_str();
        let final_state = match operation {
            "destroy" => "destroyed",
            "transfer" => "transferred",
            "replace_unique" => "replaced",
            "claim" => "claimed",
            "settle" => "settled",
            _ => "consumed",
        };
        ownership.push(TypedSemanticOwnership {
            binding: pattern.binding.clone(),
            ty: param_types.get(pattern.binding.as_str()).cloned().unwrap_or_else(|| "cell".to_string()),
            operation: operation.to_string(),
            initial_state: "available".to_string(),
            final_state: final_state.to_string(),
        });
    }
    for pattern in &body.read_refs {
        ownership.push(TypedSemanticOwnership {
            binding: pattern.binding.clone(),
            ty: param_types.get(pattern.binding.as_str()).cloned().unwrap_or_else(|| "cell".to_string()),
            operation: "read_ref".to_string(),
            initial_state: "available".to_string(),
            final_state: "available".to_string(),
        });
    }
    for pattern in &body.mutate_set {
        ownership.push(TypedSemanticOwnership {
            binding: pattern.binding.clone(),
            ty: pattern.ty.clone(),
            operation: "mutate".to_string(),
            initial_state: "available".to_string(),
            final_state: "available".to_string(),
        });
    }
    for pattern in &body.create_set {
        ownership.push(TypedSemanticOwnership {
            binding: pattern.binding.clone(),
            ty: pattern.ty.clone(),
            operation: pattern.operation.clone(),
            initial_state: "unbound".to_string(),
            final_state: "available".to_string(),
        });
    }
    let mut typed_params = params
        .iter()
        .enumerate()
        .map(|(index, param)| TypedSemanticParam {
            index: u32::try_from(index).unwrap_or(u32::MAX),
            binding_id: locals.id_for(&param.binding),
            name: param.name.clone(),
            ty: render_type(&param.ty),
            source: format!("{:?}", param.source).to_ascii_lowercase(),
            mutable: param.is_mut,
            reference: param.is_ref || param.is_read_ref,
        })
        .collect::<Vec<_>>();
    let mut typed_locals = locals.into_values();
    refine_collection_local_types(&mut typed_locals, &mut typed_params, &mut blocks);
    TypedSemanticEntry {
        id: format!("{kind}:{name}"),
        kind: kind.to_string(),
        name: name.to_string(),
        params: typed_params,
        return_type: return_type.map(render_type).unwrap_or_else(|| "unit".to_string()),
        effect: effect.to_string(),
        entry_block: body.blocks.first().and_then(|block| u32::try_from(block.id.0).ok()).unwrap_or(0),
        locals: typed_locals,
        blocks,
        borrows: body
            .borrow_regions
            .iter()
            .map(|borrow| TypedSemanticBorrow {
                root: borrow.root.clone(),
                path: borrow.path.clone(),
                binding: borrow.binding.clone(),
                root_type: borrow.root_type.clone(),
                view_type: if borrow.view_type.starts_with('&') { borrow.view_type.clone() } else { format!("&{}", borrow.view_type) },
                start_block: u32::try_from(borrow.start_block.0).unwrap_or(u32::MAX),
                start_operation: u32::try_from(borrow.start_instruction).unwrap_or(u32::MAX),
                end_block: borrow.end_block.and_then(|block| u32::try_from(block.0).ok()),
                end_operation: borrow.end_instruction.and_then(|instruction| u32::try_from(instruction).ok()),
                escapes: false,
            })
            .collect(),
        ownership,
        obligations,
    }
}

fn refine_collection_local_types(
    locals: &mut Vec<TypedSemanticLocal>,
    params: &mut [TypedSemanticParam],
    blocks: &mut [TypedSemanticBlock],
) {
    let mut candidates = BTreeMap::<u64, BTreeSet<String>>::new();
    for local in locals.iter().filter(|local| local.ty.starts_with("Vec<") && local.ty.ends_with('>')) {
        candidates.entry(local.source_id).or_default().insert(local.ty.clone());
    }
    let refinements = candidates
        .into_iter()
        .filter_map(|(source_id, types)| (types.len() == 1).then(|| (source_id, types.into_iter().next().unwrap())))
        .collect::<BTreeMap<_, _>>();
    for local in locals.iter_mut() {
        if local.ty == "Vec"
            && let Some(refined) = refinements.get(&local.source_id)
        {
            local.ty.clone_from(refined);
        }
    }
    let mut canonical_ids = BTreeMap::<(u64, String, String), u32>::new();
    let mut remapped_ids = BTreeMap::<u32, u32>::new();
    for local in locals.iter() {
        let key = (local.source_id, local.name.clone(), local.ty.clone());
        let canonical = *canonical_ids.entry(key).or_insert(local.id);
        remapped_ids.insert(local.id, canonical);
    }
    locals.retain(|local| remapped_ids.get(&local.id) == Some(&local.id));
    for param in params {
        param.binding_id = remapped_ids.get(&param.binding_id).copied().unwrap_or(param.binding_id);
    }
    let local_types = locals.iter().map(|local| (local.id, local.ty.clone())).collect::<BTreeMap<_, _>>();
    for operation in blocks.iter_mut().flat_map(|block| &mut block.operations) {
        for destination in &mut operation.destinations {
            *destination = remapped_ids.get(destination).copied().unwrap_or(*destination);
        }
        for operand in &mut operation.operands {
            if let Some(id) = operand.local {
                operand.local = Some(remapped_ids.get(&id).copied().unwrap_or(id));
            }
            if let Some(local) = operand.local.and_then(|id| local_types.get(&id)) {
                operand.ty.clone_from(local);
            }
        }
    }
}

fn operation(
    instruction: &IrInstruction,
    locals: &mut LocalTable,
    signatures: &BTreeMap<String, CallableSignature>,
) -> TypedSemanticOperation {
    let (opcode, destinations, operands, detail, call) = match instruction {
        IrInstruction::LoadConst { dest, value } => {
            let value = typed_constant(value);
            ("load-const", vec![dest], vec![], TypedSemanticOperationDetail::Constant { value }, None)
        }
        IrInstruction::LoadVar { dest, name } => {
            ("load-var", vec![dest], vec![], TypedSemanticOperationDetail::Binding { name: name.clone() }, None)
        }
        IrInstruction::StoreVar { name, src } => {
            ("store-var", vec![], vec![src], TypedSemanticOperationDetail::Binding { name: name.clone() }, None)
        }
        IrInstruction::Binary { dest, op, left, right } => (
            "binary",
            vec![dest],
            vec![left, right],
            TypedSemanticOperationDetail::BinaryOperator { operator: binary_operator_label(*op).to_string() },
            None,
        ),
        IrInstruction::Unary { dest, op, operand } => (
            "unary",
            vec![dest],
            vec![operand],
            TypedSemanticOperationDetail::UnaryOperator { operator: unary_operator_label(*op).to_string() },
            None,
        ),
        IrInstruction::FieldAccess { dest, obj, field } => {
            ("field-access", vec![dest], vec![obj], TypedSemanticOperationDetail::Field { name: field.clone() }, None)
        }
        IrInstruction::Index { dest, arr, idx } => ("index", vec![dest], vec![arr, idx], TypedSemanticOperationDetail::None, None),
        IrInstruction::Length { dest, operand } => ("length", vec![dest], vec![operand], TypedSemanticOperationDetail::None, None),
        IrInstruction::TypeHash { dest, operand } => {
            ("type-hash", vec![dest], vec![operand], TypedSemanticOperationDetail::None, None)
        }
        IrInstruction::CollectionNew { dest, ty, capacity } => (
            "collection-new",
            vec![dest],
            capacity.iter().collect(),
            TypedSemanticOperationDetail::Collection { declared_type: ty.clone() },
            None,
        ),
        IrInstruction::CollectionCapacity { dest, collection } => {
            ("collection-capacity", vec![dest], vec![collection], TypedSemanticOperationDetail::None, None)
        }
        IrInstruction::CollectionPush { collection, value } => {
            ("collection-push", vec![], vec![collection, value], TypedSemanticOperationDetail::None, None)
        }
        IrInstruction::CollectionExtend { collection, slice } => {
            ("collection-extend", vec![], vec![collection, slice], TypedSemanticOperationDetail::None, None)
        }
        IrInstruction::CollectionClear { collection } => {
            ("collection-clear", vec![], vec![collection], TypedSemanticOperationDetail::None, None)
        }
        IrInstruction::CollectionContains { dest, collection, value } => {
            ("collection-contains", vec![dest], vec![collection, value], TypedSemanticOperationDetail::None, None)
        }
        IrInstruction::CollectionRemove { dest, collection, index } => {
            ("collection-remove", vec![dest], vec![collection, index], TypedSemanticOperationDetail::None, None)
        }
        IrInstruction::CollectionInsert { collection, index, value } => {
            ("collection-insert", vec![], vec![collection, index, value], TypedSemanticOperationDetail::None, None)
        }
        IrInstruction::CollectionSet { collection, index, value } => {
            ("collection-set", vec![], vec![collection, index, value], TypedSemanticOperationDetail::None, None)
        }
        IrInstruction::CollectionPop { dest, collection } => {
            ("collection-pop", vec![dest], vec![collection], TypedSemanticOperationDetail::None, None)
        }
        IrInstruction::CollectionReverse { collection } => {
            ("collection-reverse", vec![], vec![collection], TypedSemanticOperationDetail::None, None)
        }
        IrInstruction::CollectionTruncate { collection, len } => {
            ("collection-truncate", vec![], vec![collection, len], TypedSemanticOperationDetail::None, None)
        }
        IrInstruction::CollectionSwap { collection, left, right } => {
            ("collection-swap", vec![], vec![collection, left, right], TypedSemanticOperationDetail::None, None)
        }
        IrInstruction::Call { dest, func, args } => {
            let signature = signatures.get(func).cloned().unwrap_or_else(|| CallableSignature {
                params: args.iter().map(operand_type).collect(),
                return_type: dest.as_ref().map(|dest| render_type(&dest.ty)).unwrap_or_else(|| "unit".to_string()),
                effect: "runtime-contract".to_string(),
                contract: "versioned-runtime-helper".to_string(),
            });
            (
                "call",
                dest.iter().collect(),
                args.iter().collect(),
                TypedSemanticOperationDetail::None,
                Some(TypedSemanticCall {
                    target: func.clone(),
                    params: signature.params,
                    return_type: signature.return_type,
                    effect: signature.effect,
                    contract: signature.contract,
                }),
            )
        }
        IrInstruction::ReadRef { dest, ty } => {
            ("read-ref", vec![dest], vec![], TypedSemanticOperationDetail::Reference { declared_type: ty.clone() }, None)
        }
        IrInstruction::Move { dest, src } => ("move", vec![dest], vec![src], TypedSemanticOperationDetail::None, None),
        IrInstruction::Tuple { dest, fields } => {
            ("tuple", vec![dest], fields.iter().collect(), TypedSemanticOperationDetail::None, None)
        }
        IrInstruction::EnumConstruct { dest, enum_name, variant, fields } => (
            "enum-construct",
            vec![dest],
            fields.iter().collect(),
            TypedSemanticOperationDetail::EnumConstruct { enum_name: enum_name.clone(), variant: variant.clone() },
            None,
        ),
        IrInstruction::EnumTag { dest, operand, enum_name } => {
            ("enum-tag", vec![dest], vec![operand], TypedSemanticOperationDetail::EnumTag { enum_name: enum_name.clone() }, None)
        }
        IrInstruction::EnumPayload { dest, operand, enum_name, variant, field_index } => (
            "enum-payload",
            vec![dest],
            vec![operand],
            TypedSemanticOperationDetail::EnumPayload {
                enum_name: enum_name.clone(),
                variant: variant.clone(),
                field_index: u32::try_from(*field_index).unwrap_or(u32::MAX),
            },
            None,
        ),
        IrInstruction::Consume { operand } => ("consume", vec![], vec![operand], TypedSemanticOperationDetail::None, None),
        IrInstruction::Create { dest, pattern } => (
            "create",
            vec![dest],
            create_pattern_operands(pattern),
            TypedSemanticOperationDetail::Create { pattern: typed_create_pattern(pattern) },
            None,
        ),
        IrInstruction::Transfer { dest, operand, to } => {
            ("transfer", vec![dest], vec![operand, to], TypedSemanticOperationDetail::None, None)
        }
        IrInstruction::Destroy { operand, policy } => (
            "destroy",
            vec![],
            vec![operand],
            TypedSemanticOperationDetail::Destroy { policy: destruction_policy_label(policy) },
            None,
        ),
        IrInstruction::Claim { dest, receipt } => ("claim", vec![dest], vec![receipt], TypedSemanticOperationDetail::None, None),
        IrInstruction::Settle { dest, operand } => ("settle", vec![dest], vec![operand], TypedSemanticOperationDetail::None, None),
        IrInstruction::CreateUnique { dest, pattern, identity } => (
            "create-unique",
            vec![dest],
            create_pattern_operands(pattern),
            TypedSemanticOperationDetail::CreateUnique {
                pattern: typed_create_pattern(pattern),
                identity: identity_policy_label(identity),
            },
            None,
        ),
        IrInstruction::ReplaceUnique { dest, operand, pattern, identity } => {
            let mut operands = vec![operand];
            operands.extend(create_pattern_operands(pattern));
            (
                "replace-unique",
                vec![dest],
                operands,
                TypedSemanticOperationDetail::ReplaceUnique {
                    pattern: typed_create_pattern(pattern),
                    identity: identity_policy_label(identity),
                },
                None,
            )
        }
        IrInstruction::CellMetadataEquality { left, right, field } => (
            "cell-metadata-equality",
            vec![],
            vec![left, right],
            TypedSemanticOperationDetail::CellMetadata {
                field: match field {
                    ir::CellMetadataField::LockHash => "lock-hash",
                    ir::CellMetadataField::Capacity => "capacity",
                }
                .to_string(),
            },
            None,
        ),
    };
    for destination in &destinations {
        insert_var(locals, destination);
    }
    TypedSemanticOperation {
        index: 0,
        opcode: opcode.to_string(),
        destinations: destinations.iter().map(|var| locals.id_for(var)).collect(),
        operands: operands.into_iter().map(|operand| typed_operand(operand, locals)).collect(),
        detail,
        call,
    }
}

fn binary_operator_label(operator: crate::ast::BinaryOp) -> &'static str {
    match operator {
        crate::ast::BinaryOp::Add => "add",
        crate::ast::BinaryOp::Sub => "sub",
        crate::ast::BinaryOp::Mul => "mul",
        crate::ast::BinaryOp::Div => "div",
        crate::ast::BinaryOp::Mod => "mod",
        crate::ast::BinaryOp::Eq => "eq",
        crate::ast::BinaryOp::Ne => "ne",
        crate::ast::BinaryOp::Lt => "lt",
        crate::ast::BinaryOp::Le => "le",
        crate::ast::BinaryOp::Gt => "gt",
        crate::ast::BinaryOp::Ge => "ge",
        crate::ast::BinaryOp::And => "and",
        crate::ast::BinaryOp::Or => "or",
        crate::ast::BinaryOp::BitAnd => "bit-and",
        crate::ast::BinaryOp::BitOr => "bit-or",
        crate::ast::BinaryOp::BitXor => "bit-xor",
        crate::ast::BinaryOp::Shl => "shl",
        crate::ast::BinaryOp::Shr => "shr",
    }
}

fn unary_operator_label(operator: crate::ast::UnaryOp) -> &'static str {
    match operator {
        crate::ast::UnaryOp::Neg => "neg",
        crate::ast::UnaryOp::Not => "not",
        crate::ast::UnaryOp::Ref => "ref",
        crate::ast::UnaryOp::Deref => "deref",
    }
}

fn create_pattern_operands(pattern: &ir::CreatePattern) -> Vec<&IrOperand> {
    pattern.fields.iter().map(|(_, operand)| operand).chain(pattern.lock.iter()).collect()
}

fn typed_create_pattern(pattern: &ir::CreatePattern) -> TypedSemanticCreatePattern {
    TypedSemanticCreatePattern {
        operation: pattern.operation.clone(),
        ty: pattern.ty.clone(),
        binding: pattern.binding.clone(),
        field_names: pattern.fields.iter().map(|(name, _)| name.clone()).collect(),
        has_lock: pattern.lock.is_some(),
        identity: identity_policy_label(&pattern.identity),
    }
}

fn identity_policy_label(identity: &ir::IrIdentityPolicy) -> String {
    match identity {
        ir::IrIdentityPolicy::None => "none".to_string(),
        ir::IrIdentityPolicy::CkbTypeId => "ckb-type-id".to_string(),
        ir::IrIdentityPolicy::Field(path) => format!("field:{path}"),
        ir::IrIdentityPolicy::ScriptArgs => "script-args".to_string(),
        ir::IrIdentityPolicy::SingletonType => "singleton-type".to_string(),
    }
}

fn destruction_policy_label(policy: &ir::IrDestructionPolicy) -> String {
    match policy {
        ir::IrDestructionPolicy::Default => "default".to_string(),
        ir::IrDestructionPolicy::SingletonType => "singleton-type".to_string(),
        ir::IrDestructionPolicy::Unique { identity } => format!("unique:{identity}"),
        ir::IrDestructionPolicy::Instance { identity_field } => format!("instance:{identity_field}"),
        ir::IrDestructionPolicy::BurnAmount { field } => format!("burn-amount:{field}"),
    }
}

fn typed_operand(operand: &IrOperand, locals: &mut LocalTable) -> TypedSemanticOperand {
    match operand {
        IrOperand::Var(var) => TypedSemanticOperand { local: Some(locals.id_for(var)), ty: render_type(&var.ty), constant: None },
        IrOperand::Const(value) => {
            TypedSemanticOperand { local: None, ty: render_type(&const_type(value)), constant: Some(typed_constant(value)) }
        }
    }
}

fn typed_constant(value: &ir::IrConst) -> TypedSemanticConstant {
    match value {
        ir::IrConst::Unit => TypedSemanticConstant::Unit,
        ir::IrConst::U8(value) => TypedSemanticConstant::U8(value.to_string()),
        ir::IrConst::U16(value) => TypedSemanticConstant::U16(value.to_string()),
        ir::IrConst::U32(value) => TypedSemanticConstant::U32(value.to_string()),
        ir::IrConst::U64(value) => TypedSemanticConstant::U64(value.to_string()),
        ir::IrConst::U128(value) => TypedSemanticConstant::U128(value.to_string()),
        ir::IrConst::Bool(value) => TypedSemanticConstant::Bool(*value),
        ir::IrConst::Address(value) => TypedSemanticConstant::Address(hex::encode(value)),
        ir::IrConst::Hash(value) => TypedSemanticConstant::Hash(hex::encode(value)),
        ir::IrConst::Array(values) => TypedSemanticConstant::Array(values.iter().map(typed_constant).collect()),
    }
}

fn operand_type(operand: &IrOperand) -> String {
    match operand {
        IrOperand::Var(var) => render_type(&var.ty),
        IrOperand::Const(value) => render_type(&const_type(value)),
    }
}

fn const_type(value: &ir::IrConst) -> IrType {
    match value {
        ir::IrConst::Unit => IrType::Unit,
        ir::IrConst::U8(_) => IrType::U8,
        ir::IrConst::U16(_) => IrType::U16,
        ir::IrConst::U32(_) => IrType::U32,
        ir::IrConst::U64(_) => IrType::U64,
        ir::IrConst::U128(_) => IrType::U128,
        ir::IrConst::Bool(_) => IrType::Bool,
        ir::IrConst::Address(_) => IrType::Address,
        ir::IrConst::Hash(_) => IrType::Hash,
        ir::IrConst::Array(items) => IrType::Array(Box::new(items.first().map(const_type).unwrap_or(IrType::Unit)), items.len()),
    }
}

#[derive(Default)]
struct LocalTable {
    values: BTreeMap<u32, TypedSemanticLocal>,
    identities: BTreeMap<(usize, String, String), u32>,
    next_synthetic: u32,
}

impl LocalTable {
    fn id_for(&mut self, var: &IrVar) -> u32 {
        let ty = render_type(&var.ty);
        let identity = (var.id, var.name.clone(), ty.clone());
        if let Some(id) = self.identities.get(&identity) {
            return *id;
        }
        let preferred = u32::try_from(var.id).unwrap_or(u32::MAX);
        let id = if self.values.contains_key(&preferred) { self.next_available_synthetic() } else { preferred };
        self.values
            .insert(id, TypedSemanticLocal { id, source_id: u64::try_from(var.id).unwrap_or(u64::MAX), name: var.name.clone(), ty });
        self.identities.insert(identity, id);
        id
    }

    fn next_available_synthetic(&mut self) -> u32 {
        loop {
            let candidate = 0x8000_0000_u32.saturating_add(self.next_synthetic);
            self.next_synthetic = self.next_synthetic.saturating_add(1);
            if !self.values.contains_key(&candidate) {
                return candidate;
            }
        }
    }

    fn into_values(self) -> Vec<TypedSemanticLocal> {
        self.values.into_values().collect()
    }
}

fn insert_var(locals: &mut LocalTable, var: &IrVar) {
    locals.id_for(var);
}

pub(crate) fn render_type(ty: &IrType) -> String {
    match ty {
        IrType::U8 => "u8".to_string(),
        IrType::U16 => "u16".to_string(),
        IrType::U32 => "u32".to_string(),
        IrType::I32 => "i32".to_string(),
        IrType::U64 => "u64".to_string(),
        IrType::U128 => "u128".to_string(),
        IrType::Bool => "bool".to_string(),
        IrType::Unit => "unit".to_string(),
        IrType::Address => "address".to_string(),
        IrType::Hash => "hash".to_string(),
        IrType::Array(inner, size) => format!("[{}; {}]", render_type(inner), size),
        IrType::Tuple(items) => format!("({})", items.iter().map(render_type).collect::<Vec<_>>().join(", ")),
        IrType::Named(name) => name.clone(),
        IrType::Ref(inner) => format!("&{}", render_type(inner)),
        IrType::MutRef(inner) => format!("&mut {}", render_type(inner)),
    }
}

fn proof_ids(metadata: &CompileMetadata, kind: &str, name: &str) -> Vec<String> {
    let count = match kind {
        "action" => metadata.actions.iter().find(|item| item.name == name).map(|item| item.proof_plan.len()),
        "lock" => metadata.locks.iter().find(|item| item.name == name).map(|item| item.proof_plan.len()),
        "helper" => metadata.functions.iter().find(|item| item.name == name).map(|item| item.proof_plan.len()),
        _ => None,
    }
    .unwrap_or(0);
    (0..count).map(|index| format!("proof:{kind}:{name}:{index:05}")).collect()
}
