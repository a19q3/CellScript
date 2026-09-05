//! Edition 2027's concise authoring route must retain the Edition 2026 language.
//!
//! These are differential compiler and bounded CKB-VM checks, not evidence of
//! shared-policy dispatch or a production deployment. Native preview containers
//! have a separate grammar and are deliberately outside this compatibility corpus.

use cellscript::{
    compile_file_with_entry_action, compile_with_executable_surface_policy, frontend, strip_vm_abi_trailer, CellScriptEdition,
    CompileOptions, CompileResult, EntryWitnessArg, EvidenceTier, ExecutableSurfacePolicy,
};
use ckb_testtool::{
    ckb_types::{bytes::Bytes, core::TransactionBuilder, packed, prelude::*},
    context::Context,
};

#[path = "support/ckb_script_runner.rs"]
#[allow(dead_code)]
mod ckb_script_runner;

use ckb_script_runner::{build_simple_fixture, deterministic_always_success_lock_hash, execute_cellscript_script};

const EDITIONS: [CellScriptEdition; 2] = [CellScriptEdition::Edition2026, CellScriptEdition::Edition2027];
const READ_HELPER: &str = include_str!("syntax_combo/seeds/function-effect-signature.cell");
const GENERICS: &str = include_str!("syntax_combo/seeds/generic-value.cell");
const PATTERNS: &str = include_str!("syntax_combo/seeds/complete-patterns.cell");
const BORROWS: &str = include_str!("syntax_combo/seeds/explicit-borrow.cell");
const BITWISE: &str = include_str!("syntax_combo/seeds/bitwise-shift.cell");

const CONTROL_FLOW: &str = r#"
module authoring_parity::control_flow

action verify(expected: u64) -> u64 {
    verification
        let mut total: u64 = 0
        label outer: for i in 0..4 {
            for j in 0..4 {
                if j == 0 { continue }
                if i == 2 { break outer }
                total += 1
            }
        }
        require {
            total == 6
            expected > 0
        }
        require expected == total, "wrong total"
        return 0
}
"#;

const MULTIPLE_ACTIONS: &str = r#"
module authoring_parity::multiple_actions

private fn expected() -> u64 { 7 }

action first(value: u64) -> u64 {
    verification
        require value == expected() else WrongFirst
        return 0
}

action second(witness value: u64) -> u64 {
    verification
        require value == 9 else WrongSecond
        return 0
}
"#;

const LEGACY_SUCCESSOR: &str = r#"
module authoring_parity::successor
resource Token has store, replace, relock { amount: u64 }

action transfer(input token: Token, witness recipient: Address) -> next: Token {
    verification
        require token.amount > 0
        std::lifecycle::transfer(token, next, recipient) { amount }
        std::cell::preserve_capacity(next, token)
}
"#;

// This fixture tests Lock parameter sources, not owner authentication. The
// witness equality itself is intentionally not described as a signature proof.
const LOCK_BOUNDARY: &str = r#"
module authoring_parity::lock_boundary
resource Token { owner: Address }

lock owner_guard(protected token: Token, lock_args owner: Address, witness claimed: Address) -> bool {
    verification
        require owner == token.owner
        require claimed == token.owner
}
"#;

const BOUNDED_INPUTS: &str = r#"
module authoring_parity::bounded_inputs
resource Token has store, consume { amount: u64 }

action verify(input inputs: BoundedCellSet<Token, 2>) -> u64 {
    verification
        consume_each token in inputs { require token.amount > 0 }
        return 0
}
"#;

const BOUNDED_OUTPUTS: &str = r#"
module authoring_parity::bounded_outputs
struct Plan { owner: Address, amount: u64 }
resource Token has store, create with_capacity_floor(10000000000) { amount: u64 }

action mint(witness plans: BoundedList<Plan, 2>) -> u64 {
    verification
        create_each plan in plans {
            require plan.amount > 0
            create Token { amount: plan.amount } with_lock(plan.owner)
        }
        return 0
}
"#;

fn options(edition: CellScriptEdition) -> CompileOptions {
    CompileOptions { edition, target: Some("riscv64-elf".to_string()), target_profile: Some("ckb".to_string()), ..Default::default() }
}

fn compile_source(source: &str, edition: CellScriptEdition, policy: ExecutableSurfacePolicy) -> CompileResult {
    compile_with_executable_surface_policy(source, options(edition), policy)
        .unwrap_or_else(|error| panic!("{edition:?} failed to compile parity source: {error}\n{source}"))
}

fn assert_compilation_parity(stable: &CompileResult, authoring: &CompileResult) {
    stable.validate().expect("valid Edition 2026 compiler bundle");
    authoring.validate().expect("valid Edition 2027 compiler bundle");
    assert_eq!(&stable.artifact_bytes[..4], b"\x7fELF");
    assert_eq!(strip_vm_abi_trailer(&stable.artifact_bytes), strip_vm_abi_trailer(&authoring.artifact_bytes));
    assert_ne!(stable.metadata.compatibility_profile.id, authoring.metadata.compatibility_profile.id);
    let mut before = stable.metadata.typed_semantics.clone();
    let mut after = authoring.metadata.typed_semantics.clone();
    // The public interface binds the source edition. Compare every typed
    // semantic field except that deliberately edition-specific identity.
    before.interface_hash.clear();
    after.interface_hash.clear();
    assert_eq!(before.foundation.legacy_nodes, after.foundation.legacy_nodes, "legacy operation meaning changed");
    assert_eq!(before, after);
    // Compare the actual obligations and runtime/ABI contract, in addition to
    // semantic identifiers and generated machine code.
    for (name, before, after) in [
        ("actions", serde_json::to_value(&stable.metadata.actions), serde_json::to_value(&authoring.metadata.actions)),
        ("functions", serde_json::to_value(&stable.metadata.functions), serde_json::to_value(&authoring.metadata.functions)),
        ("locks", serde_json::to_value(&stable.metadata.locks), serde_json::to_value(&authoring.metadata.locks)),
        ("runtime", serde_json::to_value(&stable.metadata.runtime), serde_json::to_value(&authoring.metadata.runtime)),
    ] {
        assert_eq!(before.unwrap(), after.unwrap(), "{name} contract changed between editions");
    }
    let before = &stable.metadata.constraints;
    let mut after = authoring.metadata.constraints.clone();
    assert_eq!(before.edition, stable.metadata.edition);
    assert_eq!(after.edition, authoring.metadata.edition);
    assert_eq!(before.compatibility_profile, stable.metadata.compatibility_profile.id);
    assert_eq!(after.compatibility_profile, authoring.metadata.compatibility_profile.id);
    after.edition = before.edition;
    after.compatibility_profile.clone_from(&before.compatibility_profile);
    assert_eq!(serde_json::to_value(before).unwrap(), serde_json::to_value(after).unwrap(), "runtime constraints changed");
}

fn compile_pair(source: &str, policy: ExecutableSurfacePolicy) -> [CompileResult; 2] {
    let results = EDITIONS.map(|edition| compile_source(source, edition, policy));
    assert_compilation_parity(&results[0], &results[1]);
    results
}

fn witness(result: &CompileResult, args: &[EntryWitnessArg]) -> Bytes {
    let payload = result.metadata.actions[0].entry_witness_args(args).expect("encode declared entry arguments");
    packed::WitnessArgs::new_builder().input_type(Some(Bytes::from(payload)).pack()).build().as_bytes()
}

#[test]
fn legacy_feature_families_keep_executable_and_obligation_parity() {
    for source in [READ_HELPER, GENERICS, PATTERNS, BORROWS, BITWISE, CONTROL_FLOW, LEGACY_SUCCESSOR, LOCK_BOUNDARY] {
        compile_pair(source, ExecutableSurfacePolicy::AllowFailClosed);
    }
}

#[test]
fn parser_and_formatter_preserve_full_action_bodies_and_declarations() {
    for source in
        [READ_HELPER, GENERICS, PATTERNS, BORROWS, CONTROL_FLOW, MULTIPLE_ACTIONS, LEGACY_SUCCESSOR, LOCK_BOUNDARY, BOUNDED_INPUTS]
    {
        let stable = frontend::parse(source, EDITIONS[0]).expect("legacy parser accepts corpus member");
        let authoring = frontend::parse(source, EDITIONS[1]).expect("authoring parser accepts corpus member");
        let formatted = cellscript::fmt::format_default(&stable).unwrap();
        assert_eq!(formatted, cellscript::fmt::format_default(&authoring).unwrap());
        let reparsed = frontend::parse(&formatted, EDITIONS[1]).expect("authoring formatter roundtrip");
        assert_eq!(formatted, cellscript::fmt::format_default(&reparsed).unwrap());
    }
}

#[test]
fn read_qualifier_and_generic_instantiations_are_not_lost() {
    let [_, read] = compile_pair(READ_HELPER, ExecutableSurfacePolicy::AllowFailClosed);
    let cellscript::ast::Item::Action(action) = read.ast.items.last().unwrap() else { panic!("inspect action") };
    assert!(action.params[0].is_read_ref);
    assert_eq!(action.params[0].source, cellscript::ast::ParamSource::Default);
    assert!(matches!(action.params[0].ty, cellscript::ast::Type::Ref(_)));
    assert!(read.metadata.functions.iter().any(|function| function.name == "threshold"));
    assert!(read.metadata.typed_semantics.foundation.provenance.bindings.iter().any(|binding| binding.entry_id == "action:inspect"));

    let [_, generic] = compile_pair(GENERICS, ExecutableSurfacePolicy::AllowFailClosed);
    assert!(generic.metadata.generic_instantiations.iter().any(|item| item.kind == "struct" && item.template == "Pair"));
    assert!(generic.metadata.generic_instantiations.iter().any(|item| item.kind == "function" && item.template == "first"));
    assert!(generic.metadata.enum_layouts.iter().any(|layout| layout.variants.iter().any(|variant| variant.name == "Some")));
}

#[test]
fn lock_parameter_sources_keep_the_existing_checked_boundary() {
    let [_, result] = compile_pair(LOCK_BOUNDARY, ExecutableSurfacePolicy::AllowFailClosed);
    let cellscript::ast::Item::Lock(lock) = result.ast.items.last().unwrap() else { panic!("owner_guard lock") };
    assert_eq!(
        lock.params.iter().map(|param| param.source).collect::<Vec<_>>(),
        [cellscript::ast::ParamSource::Protected, cellscript::ast::ParamSource::LockArgs, cellscript::ast::ParamSource::Witness]
    );
    assert_eq!(result.metadata.locks.len(), 1);
    assert_eq!(result.metadata.typed_semantics.foundation.entry_contract.script_role, "lock");
    assert_eq!(result.metadata.typed_semantics.foundation.entry_contract.trigger, "lock-group");
}

#[test]
fn multiple_source_actions_keep_explicit_selected_entry_and_distinct_code() {
    let directory = tempfile::tempdir().unwrap();
    let source_path = directory.path().join("contract.cell");
    std::fs::write(&source_path, MULTIPLE_ACTIONS).unwrap();
    let source_path = camino::Utf8Path::from_path(&source_path).unwrap();
    let mut selected = Vec::new();
    for entry in ["first", "second"] {
        let results = EDITIONS.map(|edition| {
            compile_file_with_entry_action(source_path, options(edition), entry)
                .unwrap_or_else(|error| panic!("selected {entry} in {edition:?}: {error}"))
        });
        assert_compilation_parity(&results[0], &results[1]);
        assert_eq!(results[1].metadata.actions.len(), 1);
        assert_eq!(results[1].metadata.actions[0].name, entry);
        assert_eq!(results[1].metadata.typed_semantics.foundation.entry_contract.exact_entry, format!("action:{entry}"));
        selected.push(results[1].artifact_bytes.clone());
    }
    assert_ne!(strip_vm_abi_trailer(&selected[0]), strip_vm_abi_trailer(&selected[1]));
}

#[test]
fn custom_require_and_loop_control_accept_and_reject_the_same_transactions() {
    let results = compile_pair(CONTROL_FLOW, ExecutableSurfacePolicy::DenyFailClosed);
    for expected in [6, 7] {
        let exits = results.each_ref().map(|result| {
            let mut fixture = build_simple_fixture(Bytes::default(), 1, 1);
            fixture.witnesses = vec![witness(result, &[EntryWitnessArg::U64(expected)])];
            execute_cellscript_script(strip_vm_abi_trailer(&result.artifact_bytes), &fixture).exit_code
        });
        assert_eq!(exits[0], exits[1]);
        assert_eq!(exits[0] == 0, expected == 6, "loop and custom require must actually execute");
    }
    let formatted = cellscript::fmt::format_default(&results[1].ast).unwrap();
    assert!(formatted.contains("\"wrong total\""));
    assert!(results[1].metadata.typed_semantics.foundation.claims.iter().any(|claim| claim.execution.is_some()));
}

#[test]
fn bounded_input_runtime_preserves_cardinality_decode_and_predicates() {
    let results = compile_pair(BOUNDED_INPUTS, ExecutableSurfacePolicy::DenyFailClosed);
    for result in &results {
        let action = &result.metadata.actions[0];
        assert!(action.fail_closed_runtime_features.is_empty());
        assert!(action.ckb_runtime_features.iter().any(|feature| feature == "ckb-bounded-type-group-inputs-v1"));
        assert!(action.proof_plan.iter().any(|proof| {
            proof.feature.starts_with("consume_each:") && proof.evidence_tier == EvidenceTier::CheckedRuntime && proof.on_chain_checked
        }));
    }
    for (amounts, expected_exit) in [(&[1, 2][..], 0), (&[1, 0][..], 5), (&[1, 2, 3][..], 21)] {
        for result in &results {
            let mut fixture = build_simple_fixture(Bytes::default(), amounts.len(), 1);
            fixture.current_type_script_input_indices = (0..amounts.len()).collect();
            for (input, amount) in fixture.inputs.iter_mut().zip(amounts) {
                input.data = Bytes::copy_from_slice(&u64::to_le_bytes(*amount));
            }
            let execution = execute_cellscript_script(strip_vm_abi_trailer(&result.artifact_bytes), &fixture);
            assert_eq!(execution.exit_code, expected_exit, "{amounts:?}: {:?}", execution.captured_debug);
        }
    }
    for result in &results {
        let mut fixture = build_simple_fixture(Bytes::default(), 1, 1);
        fixture.current_type_script_input_indices = vec![0];
        fixture.inputs[0].data = Bytes::from_static(&[1, 0, 0, 0]);
        assert_eq!(execute_cellscript_script(strip_vm_abi_trailer(&result.artifact_bytes), &fixture).exit_code, 4);
    }
}

#[test]
fn bounded_output_runtime_preserves_witness_plan_data_and_coverage() {
    let results = compile_pair(BOUNDED_OUTPUTS, ExecutableSurfacePolicy::DenyFailClosed);
    let mut element = deterministic_always_success_lock_hash().to_vec();
    element.extend_from_slice(&7_u64.to_le_bytes());
    let plan = cellscript::encode_bounded_output_plan_v1(&[element], 40, 2).unwrap();
    for result in &results {
        let action = &result.metadata.actions[0];
        assert!(action.fail_closed_runtime_features.is_empty());
        assert!(action.ckb_runtime_features.iter().any(|feature| feature == "ckb-bounded-output-plan-v1"));
        assert!(action.proof_plan.iter().any(|proof| {
            proof.feature.starts_with("create_each:")
                && proof.evidence_tier == EvidenceTier::CheckedRuntime
                && proof.input_output_relation_checks == ["plan_count=group_output_count<=2"]
        }));
        for (amounts, expected_exit) in [(&[7][..], 0), (&[8][..], 3), (&[7, 7][..], 21)] {
            let mut fixture = build_simple_fixture(Bytes::default(), 1, amounts.len());
            fixture.witnesses = vec![witness(result, &[EntryWitnessArg::Bytes(plan.clone())])];
            for (output, amount) in fixture.outputs.iter_mut().zip(amounts) {
                output.data = Bytes::copy_from_slice(&u64::to_le_bytes(*amount));
            }
            let execution = execute_cellscript_script(strip_vm_abi_trailer(&result.artifact_bytes), &fixture);
            assert_eq!(execution.exit_code, expected_exit, "{amounts:?}: {:?}", execution.captured_debug);
        }
    }
}

#[test]
fn existing_type_effect_and_linear_rejections_survive_the_new_frontend() {
    for (source, expected) in [
        (include_str!("syntax_combo/seeds/bounded-collection-duplicate-consume-reject.cell"), "already Consumed"),
        (include_str!("syntax_combo/seeds/bounded-collection-vec-resource-reject.cell"), "cannot store a cell-backed resource"),
        (include_str!("syntax_combo/seeds/require-block-lifecycle.cell"), "require block"),
        (include_str!("syntax_combo/seeds/explicit-borrow-cross-consume-reject.cell"), "cannot cross consume"),
        (include_str!("syntax_combo/seeds/payload-enum-nonexhaustive-reject.cell"), "non-exhaustive"),
    ] {
        let errors = EDITIONS.map(|edition| {
            compile_with_executable_surface_policy(source, options(edition), ExecutableSurfacePolicy::AllowFailClosed)
                .expect_err("invalid old language must not be accepted by the new frontend")
        });
        assert!(errors[0].message.contains(expected), "expected {expected:?}: {}", errors[0].message);
        assert_eq!(errors[0].message, errors[1].message);
        assert_eq!(errors[0].code, errors[1].code);
        assert_eq!(errors[0].span, errors[1].span);
    }
}

#[test]
fn unsupported_collection_layout_retains_production_rejection() {
    let source = BOUNDED_INPUTS.replace("amount: u64 }", "amount: u64, memo: String }");
    for edition in EDITIONS {
        let error = compile_with_executable_surface_policy(&source, options(edition), ExecutableSurfacePolicy::DenyFailClosed)
            .expect_err("dynamic bounded element remains outside the executable contract");
        assert_eq!(error.code.as_deref(), Some("E2105"));
        assert!(error.message.contains("consume_each"));
        assert!(error.message.contains("gap:runtime-helper-required"));
    }
}

#[test]
fn recovering_parser_preserves_multiple_errors_and_source_spans() {
    let source =
        "module bad\naction first() { verification let = 1\n require true }\naction second() { verification let = 2\n require true }";
    let errors = EDITIONS.map(|edition| frontend::parse_diagnostics(source, edition).expect_err("two malformed declarations"));
    assert!(errors[0].len() >= 2);
    assert_eq!(errors[0].len(), errors[1].len());
    for (before, after) in errors[0].iter().zip(&errors[1]) {
        assert_eq!(before.message, after.message);
        assert_eq!(before.span, after.span);
        assert_ne!(after.span, cellscript::error::Span::default());
    }
}

fn assert_markerless_executable_parity(marked: &CompileResult, candidate: &CompileResult) {
    marked.validate().unwrap();
    candidate.validate().unwrap();
    assert_eq!(strip_vm_abi_trailer(&marked.artifact_bytes), strip_vm_abi_trailer(&candidate.artifact_bytes));
    // Typed operations, claim execution bindings, and ABI records exclude
    // changing source spans. Compare them directly rather than erasing spans
    // recursively from the larger metadata/source-map bundle.
    assert_eq!(marked.metadata.typed_semantics.types, candidate.metadata.typed_semantics.types);
    assert_eq!(marked.metadata.typed_semantics.entries, candidate.metadata.typed_semantics.entries);
    assert_eq!(marked.metadata.typed_semantics.foundation, candidate.metadata.typed_semantics.foundation);
    assert_eq!(
        serde_json::to_value(&marked.metadata.constraints.entry_abi).unwrap(),
        serde_json::to_value(&candidate.metadata.constraints.entry_abi).unwrap()
    );
    assert_eq!(cellscript::fmt::format_default(&marked.ast).unwrap(), cellscript::fmt::format_default(&candidate.ast).unwrap());
}

#[test]
fn markerless_action_require_keeps_custom_errors_and_vm_behavior_after_formatting() {
    let marked_source = r#"
module authoring_parity::markerless_action
action verify(witness amount: u64) -> u64 {
    verification
    require amount > 0, "positive amount required"
    require amount <= 10 else AboveLimit
    return 0
}
"#;
    let markerless_source = marked_source.replacen("    verification\n", "", 1);
    assert!(frontend::parse(&markerless_source, EDITIONS[0]).is_err(), "Edition 2026 still requires its verification marker");
    let marked = compile_source(marked_source, EDITIONS[0], ExecutableSurfacePolicy::DenyFailClosed);
    let markerless = compile_source(&markerless_source, EDITIONS[1], ExecutableSurfacePolicy::DenyFailClosed);
    assert_ne!(marked.metadata.source_content_hash, markerless.metadata.source_content_hash);
    let formatted = cellscript::fmt::format_default(&markerless.ast).unwrap();
    let reparsed = compile_source(&formatted, EDITIONS[1], ExecutableSurfacePolicy::DenyFailClosed);

    let cellscript::ast::Item::Action(action) = markerless.ast.items.last().unwrap() else { panic!("verify action") };
    for (statement, expected_message) in action.body.iter().take(2).zip(["positive amount required", "AboveLimit"]) {
        let cellscript::ast::Stmt::Expr(cellscript::ast::Expr::Require(require)) = statement else {
            panic!("custom require must remain an executable condition")
        };
        let Some(cellscript::ast::Expr::String(message)) = require.message.as_deref() else { panic!("custom require message") };
        assert_eq!(message, expected_message);
    }
    assert_eq!(
        markerless.metadata.typed_semantics.foundation.claims.iter().filter(|claim| claim.execution.is_some()).count(),
        2,
        "both conditions must retain execution bindings"
    );
    for candidate in [&marked, &markerless, &reparsed] {
        assert_markerless_executable_parity(&marked, candidate);
        for (amount, expected_exit) in [(5, 0), (0, 5), (11, 5)] {
            let mut fixture = build_simple_fixture(Bytes::default(), 1, 1);
            fixture.witnesses = vec![witness(candidate, &[EntryWitnessArg::U64(amount)])];
            let result = execute_cellscript_script(strip_vm_abi_trailer(&candidate.artifact_bytes), &fixture);
            assert_eq!(result.exit_code, expected_exit, "amount={amount}: {:?}", result.captured_debug);
        }
    }
}

#[test]
fn markerless_lock_bool_tail_runs_as_a_lock_and_survives_formatting() {
    // A controlled predicate fixture, not an ownership-authentication policy.
    let marked_source = r#"
module authoring_parity::markerless_lock
lock permit(witness value: u64) -> bool {
    verification
    value == 7
}
"#;
    let markerless_source = marked_source.replacen("    verification\n", "", 1);
    assert!(frontend::parse(&markerless_source, EDITIONS[0]).is_err());
    let marked = compile_source(marked_source, EDITIONS[0], ExecutableSurfacePolicy::DenyFailClosed);
    let markerless = compile_source(&markerless_source, EDITIONS[1], ExecutableSurfacePolicy::DenyFailClosed);
    let formatted = cellscript::fmt::format_default(&markerless.ast).unwrap();
    let reparsed = compile_source(&formatted, EDITIONS[1], ExecutableSurfacePolicy::DenyFailClosed);
    let cellscript::ast::Item::Lock(lock) = markerless.ast.items.last().unwrap() else { panic!("permit lock") };
    assert!(matches!(lock.body.as_slice(), [cellscript::ast::Stmt::Expr(cellscript::ast::Expr::Binary(_))]));

    for candidate in [&marked, &markerless, &reparsed] {
        assert_markerless_executable_parity(&marked, candidate);
        assert_eq!(candidate.metadata.typed_semantics.foundation.entry_contract.script_role, "lock");
        for value in [7, 8] {
            let mut context = Context::new_with_deterministic_rng();
            let code = context.deploy_cell(Bytes::copy_from_slice(strip_vm_abi_trailer(&candidate.artifact_bytes)));
            let script = context.build_script(&code, Bytes::default()).unwrap();
            let cell = packed::CellOutput::new_builder().capacity::<packed::Uint64>(100_000_000_000_u64.pack()).lock(script).build();
            assert!(cell.type_().to_opt().is_none(), "this fixture invokes the generated program only as a Lock");
            let input = context.create_cell(cell.clone(), Bytes::default());
            let payload = candidate.metadata.locks[0].entry_witness_args(&[EntryWitnessArg::U64(value)]).unwrap();
            let witness = packed::WitnessArgs::new_builder().input_type(Some(Bytes::from(payload)).pack()).build();
            let transaction = TransactionBuilder::default()
                .input(packed::CellInput::new_builder().previous_output(input).build())
                .output(cell)
                .output_data(Bytes::default().pack())
                .witness(witness.as_bytes().pack())
                .build();
            let transaction = context.complete_tx(transaction);
            let result = context.verify_tx(&transaction, 10_000_000);
            if value == 7 {
                assert!(result.expect("true Lock tail must permit spending") > 0);
            } else {
                let error = format!("{:?}", result.expect_err("false Lock tail must reject spending"));
                assert!(error.contains("error code 5") || error.contains("error code: 5"), "wrong Lock rejection: {error}");
            }
        }
    }
}
