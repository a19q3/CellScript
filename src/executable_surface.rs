//! Compiler-owned inventory of the typed IR surface and its executable status.
//!
//! The generated Markdown and JSON matrices are checked by `cellscript-tools`.
//! Keep entries conservative: `complete` means the operation has no
//! compiler-recognized fail-closed shape; `shape-gated` means production
//! compilation accepts only shapes for which the metadata classifier reports
//! no fail-closed feature.

use serde::Serialize;

pub const EXECUTABLE_SURFACE_SCHEMA: &str = "cellscript-executable-surface-v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ExecutableSurfaceEntry {
    pub id: &'static str,
    pub layer: &'static str,
    pub status: &'static str,
    pub production_policy: &'static str,
    pub conditions: &'static str,
    pub fail_closed_features: &'static [&'static str],
}

const ACCEPT: &str = "accepted";
const ACCEPT_WHEN_CLOSED: &str = "accepted only when the shape classifier reports no fail-closed feature";
const FRONTEND_ONLY: &str = "not materialized as a runtime value";

macro_rules! entry {
    ($id:literal, $layer:literal, $status:literal, $policy:expr, $conditions:literal) => {
        ExecutableSurfaceEntry {
            id: $id,
            layer: $layer,
            status: $status,
            production_policy: $policy,
            conditions: $conditions,
            fail_closed_features: &[],
        }
    };
    ($id:literal, $layer:literal, $status:literal, $policy:expr, $conditions:literal, [$($feature:literal),+ $(,)?]) => {
        ExecutableSurfaceEntry {
            id: $id,
            layer: $layer,
            status: $status,
            production_policy: $policy,
            conditions: $conditions,
            fail_closed_features: &[$($feature),+],
        }
    };
}

pub static EXECUTABLE_SURFACE: &[ExecutableSurfaceEntry] = &[
    entry!("type:u8", "type", "complete", ACCEPT, "One-byte unsigned scalar with checked source representability."),
    entry!("type:u16", "type", "complete", ACCEPT, "Two-byte little-endian unsigned scalar."),
    entry!("type:u32", "type", "complete", ACCEPT, "Four-byte little-endian unsigned scalar."),
    entry!("type:i32", "type", "complete", ACCEPT, "Four-byte signed scalar with signed comparison, division, and remainder."),
    entry!("type:u64", "type", "complete", ACCEPT, "Eight-byte little-endian unsigned scalar."),
    entry!(
        "type:u128",
        "type",
        "bounded",
        ACCEPT_WHEN_CLOSED,
        "Sixteen-byte value with full-range decimal literals plus checked add, subtract, multiply, divide, remainder, comparison, casts, calls, parameters, and returns."
    ),
    entry!("type:bool", "type", "complete", ACCEPT, "Canonical boolean scalar."),
    entry!("type:unit", "type", "compile-time-only", FRONTEND_ONLY, "Control-flow and no-value result marker."),
    entry!("type:Address", "type", "complete", ACCEPT, "Fixed 32-byte address value."),
    entry!("type:Hash", "type", "complete", ACCEPT, "Fixed 32-byte hash value."),
    entry!("type:Array", "type", "bounded", ACCEPT_WHEN_CLOSED, "Compile-time length and recursively fixed element layout."),
    entry!(
        "type:GenericValue",
        "type",
        "bounded",
        ACCEPT_WHEN_CLOSED,
        "Struct, enum, and function templates monomorphize before IR under explicit value abilities, deterministic budgets, and hidden-Cell rejection."
    ),
    entry!(
        "type:Option",
        "type",
        "bounded",
        ACCEPT_WHEN_CLOSED,
        "Built-in Option<T> uses the ordinary fixed-width generic enum and tagged-union lowering path."
    ),
    entry!("type:Tuple", "type", "bounded", ACCEPT_WHEN_CLOSED, "Non-recursive aggregate with deterministic field offsets."),
    entry!("type:Named", "type", "shape-gated", ACCEPT_WHEN_CLOSED, "Concrete struct, enum, or Cell schema with a deterministic metadata layout."),
    entry!(
        "type:Ref",
        "type",
        "compile-time-only",
        FRONTEND_ONLY,
        "Read-only view with field-path, canonical-root reborrow, lifecycle-crossing, and non-escape checks before lowering."
    ),
    entry!("type:MutRef", "type", "reserved", "rejected by current semantic checks", "No executable general mutable-reference ABI."),
    entry!(
        "semantic:value-pattern",
        "semantic",
        "bounded",
        ACCEPT,
        "Recursive fixed enum, tuple, and struct patterns plus binding-free or-patterns with exhaustiveness and linear wildcard checks."
    ),
    entry!(
        "semantic:borrow-region",
        "semantic",
        "compile-time-only",
        FRONTEND_ONLY,
        "Field-path and reborrow regions retain one canonical Cell root and cannot materialize, escape, or cross a lifecycle operation."
    ),
    entry!(
        "semantic:loop-control",
        "semantic",
        "complete",
        ACCEPT,
        "Nearest and labeled break/continue targets lower to explicit CFG jumps after compile-time target validation."
    ),
    entry!("ir:load-const", "instruction", "complete", ACCEPT, "Materializes supported scalar and fixed-byte constants."),
    entry!("ir:load-var", "instruction", "complete", ACCEPT, "Loads a checked local binding."),
    entry!("ir:store-var", "instruction", "complete", ACCEPT, "Stores a checked local binding without changing Cell authority."),
    entry!(
        "ir:binary",
        "instruction",
        "shape-gated",
        ACCEPT_WHEN_CLOSED,
        "Scalar arithmetic, bitwise, shifts, and complete u128 operators execute directly; dynamic shifts have width guards and fixed-byte equality requires addressable operands.",
        ["fixed-byte-comparison"]
    ),
    entry!("ir:unary", "instruction", "bounded", ACCEPT, "Boolean not, scalar negation, and compile-time reference conversions."),
    entry!(
        "ir:field-access",
        "instruction",
        "shape-gated",
        ACCEPT_WHEN_CLOSED,
        "Requires a fixed schema, aggregate pointer, or tuple-call-return layout.",
        ["field-access"]
    ),
    entry!(
        "ir:index",
        "instruction",
        "shape-gated",
        ACCEPT_WHEN_CLOSED,
        "Fixed aggregates and bounded stack collections with known element layout.",
        ["index-access"]
    ),
    entry!(
        "ir:length",
        "instruction",
        "shape-gated",
        ACCEPT_WHEN_CLOSED,
        "Static lengths or validated bounded collection length words.",
        ["dynamic-length"]
    ),
    entry!(
        "ir:type-hash",
        "instruction",
        "shape-gated",
        ACCEPT_WHEN_CLOSED,
        "Schema parameter or verified output Type Script hash.",
        ["type-hash"]
    ),
    entry!(
        "ir:collection-new",
        "instruction",
        "shape-gated",
        ACCEPT_WHEN_CLOSED,
        "Bounded stack buffer or verifier-covered create-output vector.",
        ["collection-new"]
    ),
    entry!(
        "ir:collection-capacity",
        "instruction",
        "shape-gated",
        ACCEPT_WHEN_CLOSED,
        "Bounded stack collection; hidden Cell ownership is rejected.",
        ["collection-capacity", "cell-backed-collection-capacity"]
    ),
    entry!(
        "ir:collection-push",
        "instruction",
        "shape-gated",
        ACCEPT_WHEN_CLOSED,
        "Fixed-width bounded value or verified output-vector construction.",
        ["collection-push", "cell-backed-collection-push"]
    ),
    entry!(
        "ir:collection-extend",
        "instruction",
        "shape-gated",
        ACCEPT_WHEN_CLOSED,
        "Bounded fixed-width stack collection or verified output vector.",
        ["collection-extend", "cell-backed-collection-extend"]
    ),
    entry!(
        "ir:collection-clear",
        "instruction",
        "shape-gated",
        ACCEPT_WHEN_CLOSED,
        "Bounded stack collection only.",
        ["collection-clear", "cell-backed-collection-clear"]
    ),
    entry!(
        "ir:collection-contains",
        "instruction",
        "shape-gated",
        ACCEPT_WHEN_CLOSED,
        "Bounded stack collection with comparable fixed-width elements.",
        ["collection-contains", "cell-backed-collection-contains"]
    ),
    entry!(
        "ir:collection-remove",
        "instruction",
        "shape-gated",
        ACCEPT_WHEN_CLOSED,
        "Bounded stack collection with fixed-width elements.",
        ["collection-remove", "cell-backed-collection-remove"]
    ),
    entry!(
        "ir:collection-insert",
        "instruction",
        "shape-gated",
        ACCEPT_WHEN_CLOSED,
        "Bounded stack collection with checked capacity and index.",
        ["collection-insert", "cell-backed-collection-insert"]
    ),
    entry!(
        "ir:collection-set",
        "instruction",
        "shape-gated",
        ACCEPT_WHEN_CLOSED,
        "Bounded stack collection with checked index.",
        ["collection-set", "cell-backed-collection-set"]
    ),
    entry!(
        "ir:collection-pop",
        "instruction",
        "shape-gated",
        ACCEPT_WHEN_CLOSED,
        "Bounded stack collection with fixed-width result.",
        ["collection-pop", "cell-backed-collection-pop"]
    ),
    entry!(
        "ir:collection-reverse",
        "instruction",
        "shape-gated",
        ACCEPT_WHEN_CLOSED,
        "Bounded stack collection with fixed-width elements.",
        ["collection-reverse", "cell-backed-collection-reverse"]
    ),
    entry!(
        "ir:collection-truncate",
        "instruction",
        "shape-gated",
        ACCEPT_WHEN_CLOSED,
        "Bounded stack collection with checked target length.",
        ["collection-truncate", "cell-backed-collection-truncate"]
    ),
    entry!(
        "ir:collection-swap",
        "instruction",
        "shape-gated",
        ACCEPT_WHEN_CLOSED,
        "Bounded stack collection with checked indexes.",
        ["collection-swap", "cell-backed-collection-swap"]
    ),
    entry!("ir:call", "instruction", "bounded", ACCEPT_WHEN_CLOSED, "Resolved typed callable with a closed ABI and effect summary."),
    entry!("ir:read-ref", "instruction", "bounded", ACCEPT, "Explicit Input or CellDep read-only Cell view."),
    entry!("ir:move", "instruction", "complete", ACCEPT, "Typed local move; ownership validity is checked before lowering."),
    entry!("ir:tuple", "instruction", "bounded", ACCEPT_WHEN_CLOSED, "Deterministic fixed aggregate construction."),
    entry!(
        "ir:enum-construct",
        "instruction",
        "bounded",
        ACCEPT_WHEN_CLOSED,
        "Concrete fixed-width payload enum construction, including pre-IR generic enum monomorphizations."
    ),
    entry!("ir:enum-tag", "instruction", "bounded", ACCEPT_WHEN_CLOSED, "Validated concrete payload enum tag."),
    entry!("ir:enum-payload", "instruction", "bounded", ACCEPT_WHEN_CLOSED, "Fixed-width concrete enum payload field."),
    entry!(
        "ir:consume",
        "instruction",
        "shape-gated",
        ACCEPT_WHEN_CLOSED,
        "Explicit Cell-backed input consumption.",
        ["consume-expression", "non-cell-consume"]
    ),
    entry!("ir:create", "instruction", "bounded", ACCEPT_WHEN_CLOSED, "Output construction covered by create-set verification."),
    entry!(
        "ir:transfer",
        "instruction",
        "shape-gated",
        ACCEPT_WHEN_CLOSED,
        "Verifier-covered output construction and lock replacement.",
        ["transfer-expression"]
    ),
    entry!(
        "ir:destroy",
        "instruction",
        "shape-gated",
        ACCEPT_WHEN_CLOSED,
        "Explicit destructible Cell-backed operand.",
        ["destroy-expression"]
    ),
    entry!(
        "ir:claim",
        "instruction",
        "shape-gated",
        ACCEPT_WHEN_CLOSED,
        "Verifier-covered receipt claim output.",
        ["claim-expression"]
    ),
    entry!(
        "ir:settle",
        "instruction",
        "shape-gated",
        ACCEPT_WHEN_CLOSED,
        "Verifier-covered settlement output.",
        ["settle-expression"]
    ),
    entry!(
        "ir:create-unique",
        "instruction",
        "shape-gated",
        ACCEPT_WHEN_CLOSED,
        "Verifier-covered output plus executable identity policy.",
        ["create-unique-expression"]
    ),
    entry!(
        "ir:replace-unique",
        "instruction",
        "shape-gated",
        ACCEPT_WHEN_CLOSED,
        "Verifier-covered replacement plus executable identity policy.",
        ["replace-unique-expression"]
    ),
    entry!("ir:cell-metadata-equality", "instruction", "complete", ACCEPT, "Lock-hash or capacity equality over validated Cell views."),
    entry!(
        "artifact:create-output-verification",
        "artifact-policy",
        "shape-gated",
        ACCEPT_WHEN_CLOSED,
        "All constructed output fields and output lock must be materializable by the verifier.",
        ["output-verification-incomplete", "output-lock-verification-incomplete"]
    ),
    entry!(
        "artifact:cell-backed-collection-return",
        "artifact-policy",
        "reserved",
        "rejected by production policy",
        "Returning a hidden Cell-backed collection has no linear ownership ABI.",
        ["cell-backed-collection-return"]
    ),
];

pub fn executable_surface_json() -> String {
    let value = serde_json::json!({
        "schema": EXECUTABLE_SURFACE_SCHEMA,
        "entries": EXECUTABLE_SURFACE,
    });
    let mut rendered = serde_json::to_string_pretty(&value).expect("static executable surface serializes");
    rendered.push('\n');
    rendered
}

pub fn executable_surface_markdown() -> String {
    let mut rendered = String::from(
        "# CellScript Executable Surface Matrix\n\n\
**Status**: generated from the compiler-owned 0.25 executable-surface registry\n\n\
This file is generated. Run `cellscript-tools check-executable-surface --write` after changing the registry.\n\n\
Production compilation means `--production` or `--deny-fail-closed`; both stop before codegen when a selected shape reports any listed fail-closed feature. Metadata-only compilation remains available for diagnostics and Playground inspection.\n\n\
| ID | Layer | Status | Production policy | Conditions | Fail-closed features |\n\
|---|---|---|---|---|---|\n",
    );
    for entry in EXECUTABLE_SURFACE {
        let features = if entry.fail_closed_features.is_empty() { "none".to_string() } else { entry.fail_closed_features.join(", ") };
        rendered.push_str(&format!(
            "| `{}` | {} | {} | {} | {} | `{}` |\n",
            entry.id, entry.layer, entry.status, entry.production_policy, entry.conditions, features
        ));
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::EXECUTABLE_SURFACE;
    use std::collections::BTreeSet;

    #[test]
    fn registry_ids_and_fail_closed_features_are_stable_and_unique() {
        let mut ids = BTreeSet::new();
        let mut features = BTreeSet::new();
        for entry in EXECUTABLE_SURFACE {
            assert!(ids.insert(entry.id), "duplicate executable-surface ID: {}", entry.id);
            for feature in entry.fail_closed_features {
                assert!(features.insert(*feature), "fail-closed feature appears under multiple surface entries: {feature}");
                assert!(
                    feature.chars().all(|ch| ch.is_ascii_lowercase() || ch == '-'),
                    "fail-closed feature must be lowercase kebab-case: {feature}"
                );
            }
        }
    }
}
