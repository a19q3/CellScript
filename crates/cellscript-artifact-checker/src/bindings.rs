//! Parser-free consistency checks for the resolved fixed-Cell binding table.
//!
//! This checks the typed location, role and provenance projections together.
//! Machine-code equivalence remains bounded by the separate lowering checks;
//! a consistent record is not proof of arbitrary syscall dataflow.

use crate::schema::*;
use crate::{canonical_hash, CheckerError, CheckerRejectionCode};
use std::collections::{BTreeMap, BTreeSet};

fn invalid(message: String) -> CheckerError {
    CheckerError::new(CheckerRejectionCode::V2419TypedSemanticsInvalid, message)
}

fn bare_type(ty: &str) -> &str {
    let ty = ty.strip_prefix('&').map(str::trim).unwrap_or(ty);
    ty.strip_prefix("mut ").map(str::trim).unwrap_or(ty)
}

pub(crate) fn validate(typed: &TypedSemanticRecord) -> Result<(), CheckerError> {
    let graph = &typed.foundation.provenance;
    let schemas = typed.types.iter().map(|ty| (ty.name.as_str(), ty)).collect::<BTreeMap<_, _>>();
    let roles = typed.foundation.roles.iter().map(|role| (role.role_id.as_str(), role)).collect::<BTreeMap<_, _>>();
    let provenance = graph
        .bindings
        .iter()
        .map(|binding| (binding.entry_id.as_str(), binding.local_id, binding.node_id.as_str()))
        .collect::<BTreeSet<_>>();
    for entry in &typed.entries {
        if entry.cell_bindings.len() > 65_536 {
            return Err(invalid(format!("entry '{}' exceeds the fixed Cell binding budget", entry.id)));
        }
        let locals = entry.locals.iter().map(|local| (local.id, local)).collect::<BTreeMap<_, _>>();
        let binding_roles = entry.cell_bindings.iter().map(|binding| binding.role_id(&entry.id)).collect::<BTreeSet<_>>();
        let mut previous = None;
        for binding in &entry.cell_bindings {
            let key = (&binding.binding, binding.role, binding.ordinal, binding.local_id);
            if previous.is_some_and(|previous| previous >= key) {
                return Err(invalid(format!("entry '{}' has duplicate or noncanonical Cell bindings", entry.id)));
            }
            previous = Some(key);
            if binding.binding.is_empty()
                || !schemas.contains_key(binding.ty.as_str())
                || binding.ordinal == u32::MAX
                || binding.local_id.is_some_and(|id| !locals.contains_key(&id))
            {
                return Err(invalid(format!("entry '{}' has an unresolved Cell binding '{}'", entry.id, binding.binding)));
            }
            if binding.local_id.is_some_and(|id| locals.get(&id).is_none_or(|local| bare_type(&local.ty) != binding.ty)) {
                return Err(invalid(format!(
                    "entry '{}' Cell binding '{}' disagrees with its typed local schema",
                    entry.id, binding.binding
                )));
            }
            let role_matches = match binding.source {
                CellBindingSource::Input | CellBindingSource::GroupInput => {
                    matches!(binding.role, CellBindingRole::Input | CellBindingRole::ReadOnly)
                }
                CellBindingSource::Output | CellBindingSource::GroupOutput => binding.role == CellBindingRole::Output,
                CellBindingSource::CellDep => binding.role == CellBindingRole::ReadOnly,
            };
            let membership_matches = match binding.source {
                CellBindingSource::Input | CellBindingSource::Output | CellBindingSource::CellDep => {
                    binding.membership == CellBindingMembership::Unproven
                }
                CellBindingSource::GroupInput | CellBindingSource::GroupOutput => match entry.kind.as_str() {
                    "action" => binding.membership == CellBindingMembership::CurrentTypeGroup,
                    "lock" => binding.membership == CellBindingMembership::CurrentLockGroup,
                    _ => false,
                },
            };
            if !role_matches || !membership_matches {
                return Err(invalid(format!(
                    "entry '{}' Cell binding '{}' confuses source, role or Script membership",
                    entry.id, binding.binding
                )));
            }
            if entry.kind != "helper" {
                let role_id = binding.role_id(&entry.id);
                let role = roles.get(role_id.as_str());
                if role.is_none_or(|role| {
                    role.entry_id != entry.id
                        || role.binding != binding.binding
                        || role.ty != binding.ty
                        || role.direction != binding.direction()
                        || role.source != binding.source_scope()
                        || role.selector != binding.selector()
                        || role.script_identity_policy != binding.membership_policy()
                        || role.cardinality != "exactly-one"
                        || role.lock_or_type_role != if entry.kind == "lock" { "lock" } else { "type" }
                }) {
                    return Err(invalid(format!(
                        "entry '{}' Cell binding '{}' disagrees with its role projection",
                        entry.id, binding.binding
                    )));
                }
            }
            if let Some(local_id) = binding.local_id {
                let expected = canonical_hash("cellscript-value-provenance-node-v1", &binding.provenance(&entry.id))?;
                if !provenance.contains(&(entry.id.as_str(), local_id, expected.as_str())) {
                    return Err(invalid(format!(
                        "entry '{}' Cell binding '{}' disagrees with its value provenance",
                        entry.id, binding.binding
                    )));
                }
            }
        }
        if entry.kind != "helper" {
            for param in &entry.params {
                let Some(schema) = schemas.get(bare_type(&param.ty)) else { continue };
                if !param.reference && !matches!(schema.kind.as_str(), "resource" | "shared" | "receipt") {
                    continue;
                }
                if !entry.cell_bindings.iter().any(|binding| {
                    binding.binding == param.name && binding.local_id == Some(param.binding_id) && binding.ty == schema.name
                }) {
                    return Err(invalid(format!(
                        "entry '{}' fixed Cell parameter '{}' has no resolved binding",
                        entry.id, param.name
                    )));
                }
            }
            for role in typed.foundation.roles.iter().filter(|role| role.entry_id == entry.id) {
                if role.ty.starts_with("BoundedCellSet<") {
                    continue;
                }
                if !binding_roles.contains(&role.role_id) {
                    return Err(invalid(format!("entry '{}' role '{}' has no resolved Cell binding", entry.id, role.role_id)));
                }
            }
        }
        for operation in entry.blocks.iter().flat_map(|block| &block.operations).filter(|op| op.opcode == "read-ref") {
            for destination in &operation.destinations {
                let source_id = locals.get(destination).map(|local| local.source_id);
                if !entry.cell_bindings.iter().any(|binding| {
                    binding.source == CellBindingSource::CellDep
                        && binding
                            .local_id
                            .is_some_and(|root| locals.get(&root).is_some_and(|local| Some(local.source_id) == source_id))
                }) {
                    return Err(invalid(format!("entry '{}' read-ref has no resolved CellDep location", entry.id)));
                }
            }
        }
    }
    Ok(())
}
