//! Compiler emission of the versioned typed-semantic record consumed by the
//! standalone artifact checker. The checker owns validation; this module only
//! translates checked IR into the shared, parser-free schema.

use crate::ir::{self, IrInstruction, IrOperand, IrTerminator, IrType, IrVar};
use crate::CompileMetadata;
use cellscript_artifact_checker::{
    canonical_hash, TypedSemanticBlock, TypedSemanticBorrow, TypedSemanticCall, TypedSemanticEntry, TypedSemanticField,
    TypedSemanticInstantiation, TypedSemanticLocal, TypedSemanticOperand, TypedSemanticOperation, TypedSemanticOwnership,
    TypedSemanticParam, TypedSemanticRecord, TypedSemanticType, TYPED_SEMANTICS_SCHEMA,
};
use std::collections::BTreeMap;

pub(crate) fn build(module: &ir::IrModule, metadata: &CompileMetadata) -> TypedSemanticRecord {
    let mut types = module
        .external_type_defs
        .iter()
        .chain(module.items.iter().filter_map(|item| match item {
            ir::IrItem::TypeDef(definition) => Some(definition),
            _ => None,
        }))
        .map(|definition| {
            let fields = definition
                .fields
                .iter()
                .map(|field| TypedSemanticField {
                    name: field.name.clone(),
                    ty: render_type(&field.ty),
                    offset: u32::try_from(field.offset).unwrap_or(u32::MAX),
                    width_bytes: field.fixed_size.and_then(|width| u32::try_from(width).ok()),
                })
                .collect::<Vec<_>>();
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
            let capabilities = definition.capabilities.iter().map(|capability| capability.as_str().to_string()).collect::<Vec<_>>();
            let layout_hash = canonical_hash("cellscript-typed-layout-v1", &(kind, encoded_size, &fields))
                .expect("typed layout record is serializable");
            TypedSemanticType {
                name: definition.name.clone(),
                kind: kind.to_string(),
                encoded_size,
                fields,
                capabilities,
                layout_hash,
            }
        })
        .collect::<Vec<_>>();
    for layout in module.enum_layouts.values() {
        let fields = layout
            .variants
            .iter()
            .flat_map(|variant| {
                variant.fields.iter().map(move |field| TypedSemanticField {
                    name: format!("{}::{}", variant.name, field.index),
                    ty: render_type(&field.ty),
                    offset: u32::try_from(field.offset).unwrap_or(u32::MAX),
                    width_bytes: u32::try_from(field.width).ok(),
                })
            })
            .collect::<Vec<_>>();
        let encoded_size = u32::try_from(layout.encoded_size).ok();
        let layout_hash = canonical_hash("cellscript-typed-layout-v1", &("enum", encoded_size, &fields))
            .expect("typed enum layout record is serializable");
        types.push(TypedSemanticType {
            name: layout.name.clone(),
            kind: "enum".to_string(),
            encoded_size,
            fields,
            capabilities: Vec::new(),
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
            template: item.template.clone(),
            identity: item.identity.clone(),
            type_arguments: item.type_arguments.clone(),
            constraints_verified: item.constraints_verified,
        })
        .collect::<Vec<_>>();
    let mut record = TypedSemanticRecord {
        schema: TYPED_SEMANTICS_SCHEMA.to_string(),
        version: 1,
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
            if let IrTerminator::Return(Some(operand)) = &block.terminator {
                operations.push(TypedSemanticOperation {
                    opcode: "return".to_string(),
                    destinations: Vec::new(),
                    operands: vec![typed_operand(operand, &mut locals)],
                    call: None,
                });
            }
            if let IrTerminator::Branch { cond, .. } = &block.terminator {
                operations.push(TypedSemanticOperation {
                    opcode: "branch-condition".to_string(),
                    destinations: Vec::new(),
                    operands: vec![typed_operand(cond, &mut locals)],
                    call: None,
                });
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
            }
        })
        .collect::<Vec<_>>();
    blocks.sort_by_key(|block| block.id);
    let param_types = params.iter().map(|param| (param.name.as_str(), render_type(&param.ty))).collect::<BTreeMap<_, _>>();
    let mut ownership = Vec::new();
    for pattern in &body.consume_set {
        let operation = if pattern.operation.contains("destroy") { "destroy" } else { "consume" };
        ownership.push(TypedSemanticOwnership {
            binding: pattern.binding.clone(),
            ty: param_types.get(pattern.binding.as_str()).cloned().unwrap_or_else(|| "cell".to_string()),
            operation: operation.to_string(),
            initial_state: "available".to_string(),
            final_state: if operation == "destroy" { "destroyed" } else { "consumed" }.to_string(),
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
            operation: "create".to_string(),
            initial_state: "unbound".to_string(),
            final_state: "available".to_string(),
        });
    }
    let typed_params = params
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
        .collect();
    TypedSemanticEntry {
        id: format!("{kind}:{name}"),
        kind: kind.to_string(),
        name: name.to_string(),
        params: typed_params,
        return_type: return_type.map(render_type).unwrap_or_else(|| "unit".to_string()),
        effect: effect.to_string(),
        locals: locals.into_values(),
        blocks,
        borrows: body
            .borrow_regions
            .iter()
            .map(|borrow| TypedSemanticBorrow {
                root: borrow.root.clone(),
                path: borrow.path.clone(),
                binding: borrow.binding.clone(),
                root_type: borrow.root_type.clone(),
                view_type: borrow.view_type.clone(),
                escapes: false,
            })
            .collect(),
        ownership,
        obligations,
    }
}

fn operation(
    instruction: &IrInstruction,
    locals: &mut LocalTable,
    signatures: &BTreeMap<String, CallableSignature>,
) -> TypedSemanticOperation {
    let (opcode, destinations, operands, call) = match instruction {
        IrInstruction::LoadConst { dest, .. } => ("load-const", vec![dest], vec![], None),
        IrInstruction::LoadVar { dest, .. } => ("load-var", vec![dest], vec![], None),
        IrInstruction::StoreVar { src, .. } => ("store-var", vec![], vec![src], None),
        IrInstruction::Binary { dest, left, right, .. } => ("binary", vec![dest], vec![left, right], None),
        IrInstruction::Unary { dest, operand, .. } => ("unary", vec![dest], vec![operand], None),
        IrInstruction::FieldAccess { dest, obj, .. } => ("field-access", vec![dest], vec![obj], None),
        IrInstruction::Index { dest, arr, idx } => ("index", vec![dest], vec![arr, idx], None),
        IrInstruction::Length { dest, operand } => ("length", vec![dest], vec![operand], None),
        IrInstruction::TypeHash { dest, operand } => ("type-hash", vec![dest], vec![operand], None),
        IrInstruction::CollectionNew { dest, capacity, .. } => ("collection-new", vec![dest], capacity.iter().collect(), None),
        IrInstruction::CollectionCapacity { dest, collection } => ("collection-capacity", vec![dest], vec![collection], None),
        IrInstruction::CollectionPush { collection, value } => ("collection-push", vec![], vec![collection, value], None),
        IrInstruction::CollectionExtend { collection, slice } => ("collection-extend", vec![], vec![collection, slice], None),
        IrInstruction::CollectionClear { collection } => ("collection-clear", vec![], vec![collection], None),
        IrInstruction::CollectionContains { dest, collection, value } => {
            ("collection-contains", vec![dest], vec![collection, value], None)
        }
        IrInstruction::CollectionRemove { dest, collection, index } => {
            ("collection-remove", vec![dest], vec![collection, index], None)
        }
        IrInstruction::CollectionInsert { collection, index, value } => {
            ("collection-insert", vec![], vec![collection, index, value], None)
        }
        IrInstruction::CollectionSet { collection, index, value } => ("collection-set", vec![], vec![collection, index, value], None),
        IrInstruction::CollectionPop { dest, collection } => ("collection-pop", vec![dest], vec![collection], None),
        IrInstruction::CollectionReverse { collection } => ("collection-reverse", vec![], vec![collection], None),
        IrInstruction::CollectionTruncate { collection, len } => ("collection-truncate", vec![], vec![collection, len], None),
        IrInstruction::CollectionSwap { collection, left, right } => ("collection-swap", vec![], vec![collection, left, right], None),
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
                Some(TypedSemanticCall {
                    target: func.clone(),
                    params: signature.params,
                    return_type: signature.return_type,
                    effect: signature.effect,
                    contract: signature.contract,
                }),
            )
        }
        IrInstruction::ReadRef { dest, .. } => ("read-ref", vec![dest], vec![], None),
        IrInstruction::Move { dest, src } => ("move", vec![dest], vec![src], None),
        IrInstruction::Tuple { dest, fields } => ("tuple", vec![dest], fields.iter().collect(), None),
        IrInstruction::EnumConstruct { dest, fields, .. } => ("enum-construct", vec![dest], fields.iter().collect(), None),
        IrInstruction::EnumTag { dest, operand, .. } => ("enum-tag", vec![dest], vec![operand], None),
        IrInstruction::EnumPayload { dest, operand, .. } => ("enum-payload", vec![dest], vec![operand], None),
        IrInstruction::Consume { operand } => ("consume", vec![], vec![operand], None),
        IrInstruction::Create { dest, .. } => ("create", vec![dest], vec![], None),
        IrInstruction::Transfer { dest, operand, to } => ("transfer", vec![dest], vec![operand, to], None),
        IrInstruction::Destroy { operand, .. } => ("destroy", vec![], vec![operand], None),
        IrInstruction::Claim { dest, receipt } => ("claim", vec![dest], vec![receipt], None),
        IrInstruction::Settle { dest, operand } => ("settle", vec![dest], vec![operand], None),
        IrInstruction::CreateUnique { dest, .. } => ("create-unique", vec![dest], vec![], None),
        IrInstruction::ReplaceUnique { dest, operand, .. } => ("replace-unique", vec![dest], vec![operand], None),
        IrInstruction::CellMetadataEquality { left, right, .. } => ("cell-metadata-equality", vec![], vec![left, right], None),
    };
    for destination in &destinations {
        insert_var(locals, destination);
    }
    TypedSemanticOperation {
        opcode: opcode.to_string(),
        destinations: destinations.iter().map(|var| locals.id_for(var)).collect(),
        operands: operands.into_iter().map(|operand| typed_operand(operand, locals)).collect(),
        call,
    }
}

fn typed_operand(operand: &IrOperand, locals: &mut LocalTable) -> TypedSemanticOperand {
    match operand {
        IrOperand::Var(var) => TypedSemanticOperand { local: Some(locals.id_for(var)), ty: render_type(&var.ty) },
        IrOperand::Const(value) => TypedSemanticOperand { local: None, ty: render_type(&const_type(value)) },
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
        self.values.insert(id, TypedSemanticLocal { id, name: var.name.clone(), ty });
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
