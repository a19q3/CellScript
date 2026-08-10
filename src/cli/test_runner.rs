use crate::error::{CompileError, Result};
use crate::runtime_errors::CellScriptRuntimeError;
use crate::simulate::{SimValue, SimulateError, SimulateInterpreter};
use crate::{compile_path_with_entry_action, compile_path_with_entry_lock, CompileOptions, CompileResult};
use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

#[cfg(feature = "vm-runner")]
use ckb_vm::{
    cost_model::estimate_cycles, machine::VERSION2, Bytes, DefaultCoreMachine, DefaultMachineBuilder, DefaultMachineRunner,
    Error as VmError, SparseMemory, SupportMachine, TraceMachine, WXorXMemory, ISA_B, ISA_IMC, ISA_MOP,
};

const SCENARIO_SCHEMA: &str = "cellscript-test-scenario-v1";
const MAX_SCENARIO_BYTES: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TestBackend {
    Simulator,
    CkbVm,
}

impl TestBackend {
    pub(super) fn parse(value: &str) -> Result<Vec<Self>> {
        match value {
            "simulator" => Ok(vec![Self::Simulator]),
            "ckb-vm" => Ok(vec![Self::CkbVm]),
            "all" => Ok(vec![Self::Simulator, Self::CkbVm]),
            _ => Err(CompileError::without_span("invalid test backend; expected simulator, ckb-vm, or all")),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Simulator => "simulator",
            Self::CkbVm => "ckb-vm",
        }
    }

    fn evidence_tier(self) -> &'static str {
        match self {
            Self::Simulator => "development-non-consensus",
            Self::CkbVm => "authoritative-runtime",
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Scenario {
    schema: String,
    name: String,
    source: String,
    target_profile: String,
    entry: ScenarioEntry,
    initial_cells: Vec<ScenarioCell>,
    steps: Vec<ScenarioStep>,
    limits: ScenarioLimits,
    #[serde(default)]
    oracle: Option<ScenarioOracle>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScenarioEntry {
    kind: String,
    name: String,
    args: Vec<ScenarioArgument>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScenarioArgument {
    name: String,
    ty: String,
    value: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScenarioCell {
    name: String,
    capacity: u64,
    data: String,
    lock: ScenarioScript,
    #[serde(rename = "type")]
    type_script: Option<ScenarioScript>,
    #[serde(default)]
    prior_output: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScenarioScript {
    code_hash: String,
    hash_type: String,
    args: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScenarioStep {
    name: String,
    consumes: Vec<String>,
    outputs: Vec<ScenarioCell>,
    cell_deps: Vec<ScenarioCellDep>,
    header_deps: Vec<ScenarioHeaderDep>,
    since: BTreeMap<String, u64>,
    witnesses: Vec<ScenarioWitness>,
    expectation: ScenarioExpectation,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScenarioCellDep {
    name: String,
    tx_hash: String,
    index: u32,
    dep_type: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScenarioHeaderDep {
    name: String,
    hash: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScenarioWitness {
    input: String,
    lock: Option<String>,
    input_type: Option<String>,
    output_type: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScenarioExpectation {
    status: String,
    #[serde(default)]
    result: Option<String>,
    #[serde(default)]
    runtime_error: Option<ExpectedRuntimeError>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ExpectedRuntimeError {
    code: u64,
    name: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScenarioLimits {
    max_steps: u64,
    max_cycles: u64,
    max_transaction_bytes: u64,
    minimum_cell_capacity: u64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScenarioOracle {
    kind: String,
    scenario_id: String,
    evidence_path: String,
}

#[derive(Debug)]
struct StateReport {
    initial_live: Vec<String>,
    final_live: Vec<String>,
    transitions: Vec<Value>,
}

#[derive(Debug)]
enum Observation {
    Passed { result: String, steps: Option<u64>, cycles: Option<u64>, trace: Vec<String> },
    RuntimeError { code: u64, name: String, steps: Option<u64>, cycles: Option<u64>, trace: Vec<String> },
}

pub(super) fn collect_scenario_files(root: &Path) -> Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        for entry in std::fs::read_dir(&path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.ends_with(".scenario.json")) {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

pub(super) fn run_scenario(path: &Path, backend: TestBackend) -> Result<Value> {
    let bytes = std::fs::read(path)
        .map_err(|error| CompileError::without_span(format!("failed to read scenario '{}': {error}", path.display())))?;
    if bytes.is_empty() || bytes.len() as u64 > MAX_SCENARIO_BYTES {
        return Err(CompileError::without_span(format!(
            "scenario '{}' must be non-empty and no larger than {MAX_SCENARIO_BYTES} bytes",
            path.display()
        )));
    }
    let scenario: Scenario = serde_json::from_slice(&bytes)
        .map_err(|error| CompileError::without_span(format!("invalid scenario '{}': {error}", path.display())))?;
    validate_scenario_shape(path, &scenario, bytes.len() as u64)?;
    let source = resolve_scenario_source(path, &scenario.source)?;
    let state = validate_state_transitions(&scenario)?;
    validate_oracle(path, scenario.oracle.as_ref())?;

    let options = CompileOptions {
        target: Some("riscv64-elf".to_string()),
        target_profile: Some("ckb".to_string()),
        ..CompileOptions::default()
    };
    let result = match scenario.entry.kind.as_str() {
        "action" => compile_path_with_entry_action(&source, options, scenario.entry.name.clone())?,
        "lock" => compile_path_with_entry_lock(&source, options, scenario.entry.name.clone())?,
        _ => unreachable!("validated entry kind"),
    };
    let checker_report = checker_report(&result)?;

    let args = scenario.entry.args.iter().map(scenario_argument_value).collect::<Result<Vec<_>>>()?;
    let mut step_reports = Vec::with_capacity(scenario.steps.len());
    for step in &scenario.steps {
        let observation = match backend {
            TestBackend::Simulator => run_simulator(&result, &scenario, &args)?,
            TestBackend::CkbVm => run_ckb_vm(&result, &scenario)?,
        };
        validate_expectation(&step.expectation, &observation, &step.name)?;
        step_reports.push(observation_report(step, observation));
    }

    let coverage = coverage_report(&result, &scenario, backend);
    Ok(json!({
        "schema": "cellscript-test-report-v1",
        "status": "passed",
        "scenario": scenario.name,
        "scenario_path": path.to_string_lossy(),
        "backend": backend.name(),
        "evidence_tier": backend.evidence_tier(),
        "compiler_version": result.metadata.compiler_version,
        "artifact_hash": result.metadata.artifact_hash,
        "checker_name": result.metadata.verified_artifact.checker_name,
        "checker_version": checker_report.checker_version,
        "checker_policy_schema": checker_report.checker_policy_schema,
        "lowering_record_hash": result.metadata.verified_artifact.lowering_record_hash,
        "source_map_hash": result.metadata.verified_artifact.source_map_hash,
        "target_profile": result.metadata.target_profile.name,
        "compatibility_profile": result.metadata.compatibility_profile,
        "entry": {
            "kind": scenario.entry.kind,
            "name": scenario.entry.name,
            "inputs": scenario.entry.args.iter().map(|arg| json!({"name": arg.name, "type": arg.ty, "value": arg.value})).collect::<Vec<_>>()
        },
        "state": {
            "validation": "local-live-cell-replacement-v1",
            "initial_live": state.initial_live,
            "final_live": state.final_live,
            "transitions": state.transitions
        },
        "oracle": scenario.oracle.as_ref().map(|oracle| json!({
            "kind": oracle.kind,
            "scenario_id": oracle.scenario_id,
            "evidence_path": oracle.evidence_path,
            "state": "declared-not-promoted-by-package-runner"
        })),
        "steps": step_reports,
        "coverage": coverage,
    }))
}

fn validate_scenario_shape(path: &Path, scenario: &Scenario, scenario_bytes: u64) -> Result<()> {
    if scenario.schema != SCENARIO_SCHEMA || scenario.name.is_empty() || scenario.target_profile != "ckb" {
        return Err(CompileError::without_span(format!(
            "scenario '{}' has an unsupported schema, empty name, or non-CKB target profile",
            path.display()
        )));
    }
    if !matches!(scenario.entry.kind.as_str(), "action" | "lock") || scenario.entry.name.is_empty() {
        return Err(CompileError::without_span(format!("scenario '{}' has an invalid entry", path.display())));
    }
    if scenario.steps.is_empty()
        || scenario.limits.max_steps == 0
        || scenario.limits.max_cycles == 0
        || scenario.limits.max_transaction_bytes == 0
        || scenario_bytes > scenario.limits.max_transaction_bytes
    {
        return Err(CompileError::without_span(format!("scenario '{}' has empty steps or invalid/exceeded limits", path.display())));
    }
    let mut argument_names = BTreeSet::new();
    for argument in &scenario.entry.args {
        if argument.name.is_empty() || argument.ty.is_empty() || !argument_names.insert(argument.name.as_str()) {
            return Err(CompileError::without_span(format!("scenario '{}' has duplicate or empty entry arguments", path.display())));
        }
    }
    let mut step_names = BTreeSet::new();
    for step in &scenario.steps {
        if step.name.is_empty() || !step_names.insert(step.name.as_str()) {
            return Err(CompileError::without_span(format!("scenario '{}' has duplicate or empty step names", path.display())));
        }
        validate_step_contract(path, step, &scenario.limits)?;
    }
    Ok(())
}

fn validate_step_contract(path: &Path, step: &ScenarioStep, limits: &ScenarioLimits) -> Result<()> {
    let expected = &step.expectation;
    match expected.status.as_str() {
        "pass" if expected.runtime_error.is_none() => {}
        "runtime-error" => {
            let error = expected.runtime_error.as_ref().ok_or_else(|| {
                CompileError::without_span(format!("scenario '{}' step '{}' omits its exact runtime error", path.display(), step.name))
            })?;
            let registered = CellScriptRuntimeError::from_code(error.code).ok_or_else(|| {
                CompileError::without_span(format!(
                    "scenario '{}' step '{}' uses unknown runtime code {}",
                    path.display(),
                    step.name,
                    error.code
                ))
            })?;
            if registered.name() != error.name {
                return Err(CompileError::without_span(format!(
                    "scenario '{}' step '{}' runtime code/name mismatch",
                    path.display(),
                    step.name
                )));
            }
        }
        _ => {
            return Err(CompileError::without_span(format!(
                "scenario '{}' step '{}' has an invalid expectation",
                path.display(),
                step.name
            )));
        }
    }
    let estimate = serde_json::to_vec(step)
        .map_err(|error| CompileError::without_span(format!("failed to size scenario step: {error}")))?
        .len() as u64;
    if estimate > limits.max_transaction_bytes {
        return Err(CompileError::without_span(format!("scenario step '{}' exceeds max_transaction_bytes", step.name)));
    }
    let mut deps = BTreeSet::new();
    for dep in &step.cell_deps {
        if dep.name.is_empty()
            || !deps.insert(dep.name.as_str())
            || !valid_hash(&dep.tx_hash)
            || !matches!(dep.dep_type.as_str(), "code" | "dep-group")
        {
            return Err(CompileError::without_span(format!("scenario step '{}' has an invalid CellDep", step.name)));
        }
        let _ = dep.index;
    }
    let mut headers = BTreeSet::new();
    for header in &step.header_deps {
        if header.name.is_empty() || !headers.insert(header.name.as_str()) || !valid_hash(&header.hash) {
            return Err(CompileError::without_span(format!("scenario step '{}' has an invalid HeaderDep", step.name)));
        }
    }
    for cell in step.since.keys() {
        if !step.consumes.contains(cell) {
            return Err(CompileError::without_span(format!("scenario step '{}' has since for a non-consumed Cell", step.name)));
        }
    }
    let mut witnessed = BTreeSet::new();
    for witness in &step.witnesses {
        if !step.consumes.contains(&witness.input) || !witnessed.insert(witness.input.as_str()) {
            return Err(CompileError::without_span(format!("scenario step '{}' has a stale or duplicate witness input", step.name)));
        }
        for bytes in [&witness.lock, &witness.input_type, &witness.output_type].into_iter().flatten() {
            validate_hex("witness", bytes)?;
        }
    }
    Ok(())
}

fn validate_state_transitions(scenario: &Scenario) -> Result<StateReport> {
    let mut live = BTreeMap::<String, ScenarioCell>::new();
    let mut all_names = BTreeSet::new();
    for cell in &scenario.initial_cells {
        validate_cell(cell, &scenario.limits)?;
        if !all_names.insert(cell.name.clone()) || live.insert(cell.name.clone(), cell.clone()).is_some() {
            return Err(CompileError::without_span(format!("scenario '{}' has duplicate initial Cell names", scenario.name)));
        }
    }
    let initial_live = live.keys().cloned().collect();
    let mut transitions = Vec::new();
    for step in &scenario.steps {
        let mut consumed = BTreeSet::new();
        for name in &step.consumes {
            if !consumed.insert(name.clone()) || live.remove(name).is_none() {
                return Err(CompileError::without_span(format!(
                    "scenario '{}' step '{}' consumes a missing, dead, or duplicate Cell '{}'",
                    scenario.name, step.name, name
                )));
            }
        }
        let mut produced = Vec::new();
        for cell in &step.outputs {
            validate_cell(cell, &scenario.limits)?;
            if all_names.contains(&cell.name) || live.contains_key(&cell.name) {
                return Err(CompileError::without_span(format!(
                    "scenario '{}' step '{}' reuses Cell name '{}'",
                    scenario.name, step.name, cell.name
                )));
            }
            if let Some(prior) = &cell.prior_output {
                if !consumed.contains(prior) {
                    return Err(CompileError::without_span(format!(
                        "scenario '{}' step '{}' output '{}' names unconsumed prior output '{}'",
                        scenario.name, step.name, cell.name, prior
                    )));
                }
            }
            all_names.insert(cell.name.clone());
            produced.push(cell.name.clone());
            live.insert(cell.name.clone(), cell.clone());
        }
        transitions.push(json!({
            "step": step.name,
            "consumed_became_dead": consumed,
            "outputs_became_live": produced,
            "live_after": live.keys().cloned().collect::<Vec<_>>()
        }));
    }
    Ok(StateReport { initial_live, final_live: live.keys().cloned().collect(), transitions })
}

fn validate_cell(cell: &ScenarioCell, limits: &ScenarioLimits) -> Result<()> {
    if cell.name.is_empty() || cell.capacity < limits.minimum_cell_capacity {
        return Err(CompileError::without_span(format!("Cell '{}' has an empty name or insufficient capacity", cell.name)));
    }
    validate_hex("Cell data", &cell.data)?;
    validate_script(&cell.lock)?;
    if let Some(script) = &cell.type_script {
        validate_script(script)?;
    }
    Ok(())
}

fn validate_script(script: &ScenarioScript) -> Result<()> {
    if !valid_hash(&script.code_hash) || !matches!(script.hash_type.as_str(), "data" | "type" | "data1" | "data2") {
        return Err(CompileError::without_span("scenario contains an invalid Script identity"));
    }
    validate_hex("Script args", &script.args)
}

fn validate_hex(label: &str, value: &str) -> Result<()> {
    let raw = value.strip_prefix("0x").unwrap_or(value);
    if !raw.len().is_multiple_of(2) || !raw.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CompileError::without_span(format!("{label} must be even-length hexadecimal")));
    }
    Ok(())
}

fn valid_hash(value: &str) -> bool {
    let raw = value.strip_prefix("0x").unwrap_or(value);
    raw.len() == 64 && raw.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn resolve_scenario_source(scenario_path: &Path, source: &str) -> Result<Utf8PathBuf> {
    let source_path = Path::new(source);
    if source_path.is_absolute()
        || source_path
            .components()
            .any(|component| matches!(component, Component::ParentDir | Component::RootDir | Component::Prefix(_)))
    {
        return Err(CompileError::without_span("scenario source path must be confined and relative"));
    }
    let parent = scenario_path.parent().unwrap_or_else(|| Path::new("."));
    let root = std::fs::canonicalize(parent)
        .map_err(|error| CompileError::without_span(format!("failed to resolve scenario directory: {error}")))?;
    let resolved = std::fs::canonicalize(parent.join(source))
        .map_err(|error| CompileError::without_span(format!("failed to resolve scenario source '{source}': {error}")))?;
    if !resolved.starts_with(&root) || resolved.extension().and_then(|extension| extension.to_str()) != Some("cell") {
        return Err(CompileError::without_span("scenario source escapes its directory or is not a .cell file"));
    }
    Utf8PathBuf::from_path_buf(resolved)
        .map_err(|path| CompileError::without_span(format!("scenario source path '{}' is not valid UTF-8", path.display())))
}

fn validate_oracle(scenario_path: &Path, oracle: Option<&ScenarioOracle>) -> Result<()> {
    let Some(oracle) = oracle else {
        return Ok(());
    };
    if oracle.kind != "cellscript-ckb-stateful-scenario-v1" || oracle.scenario_id.is_empty() {
        return Err(CompileError::without_span("scenario oracle has an unsupported kind or empty id"));
    }
    let evidence = Path::new(&oracle.evidence_path);
    if evidence.is_absolute() || evidence.components().any(|component| matches!(component, Component::ParentDir)) {
        return Err(CompileError::without_span("scenario oracle evidence path must be confined and relative"));
    }
    let _ = scenario_path;
    Ok(())
}

fn scenario_argument_value(argument: &ScenarioArgument) -> Result<SimValue> {
    match argument.ty.as_str() {
        "u8" | "u16" | "u32" | "u64" => argument
            .value
            .as_u64()
            .map(SimValue::Integer)
            .ok_or_else(|| CompileError::without_span(format!("argument '{}' must be an unsigned integer", argument.name))),
        "bool" => argument
            .value
            .as_bool()
            .map(SimValue::Bool)
            .ok_or_else(|| CompileError::without_span(format!("argument '{}' must be a bool", argument.name))),
        "string" => argument
            .value
            .as_str()
            .map(|value| SimValue::String(value.to_string()))
            .ok_or_else(|| CompileError::without_span(format!("argument '{}' must be a string", argument.name))),
        other => Err(CompileError::without_span(format!("scenario argument type '{other}' is not supported by the v1 runner"))),
    }
}

fn run_simulator(result: &CompileResult, scenario: &Scenario, args: &[SimValue]) -> Result<Observation> {
    let mut interpreter = SimulateInterpreter::new(&result.ast, scenario.limits.max_steps);
    let observed = match scenario.entry.kind.as_str() {
        "action" => interpreter.simulate_action(&scenario.entry.name, args),
        "lock" => interpreter.simulate_lock(&scenario.entry.name, args),
        _ => unreachable!("validated entry kind"),
    };
    match observed {
        Ok(observed) => Ok(Observation::Passed {
            result: observed.return_value.to_string(),
            steps: Some(observed.steps),
            cycles: None,
            trace: observed.trace.iter().map(ToString::to_string).collect(),
        }),
        Err(SimulateError::RuntimeError { code, name }) => {
            Ok(Observation::RuntimeError { code, name, steps: None, cycles: None, trace: Vec::new() })
        }
        Err(error) => Err(CompileError::without_span(format!("scenario simulator failed: {error}"))),
    }
}

#[cfg(feature = "vm-runner")]
fn run_ckb_vm(result: &CompileResult, scenario: &Scenario) -> Result<Observation> {
    if !scenario.entry.args.is_empty() {
        return Err(CompileError::without_span(
            "ckb-vm scenario entry arguments require a transaction syscall harness; use an imported stateful oracle",
        ));
    }
    type ScenarioMachine = TraceMachine<DefaultCoreMachine<u64, WXorXMemory<SparseMemory<u64>>>>;
    let core_machine = <<ScenarioMachine as DefaultMachineRunner>::Inner as SupportMachine>::new(
        ISA_IMC | ISA_B | ISA_MOP,
        VERSION2,
        scenario.limits.max_cycles,
    );
    let builder = DefaultMachineBuilder::new(core_machine).instruction_cycle_func(Box::new(estimate_cycles));
    let mut machine = ScenarioMachine::new(builder.build());
    let program = Bytes::copy_from_slice(crate::strip_vm_abi_trailer(&result.artifact_bytes));
    machine
        .load_program(&program, std::iter::empty::<std::result::Result<Bytes, VmError>>())
        .map_err(|error| CompileError::without_span(format!("scenario CKB-VM failed to load ELF: {error}")))?;
    let exit_code = machine.run().map_err(|error| CompileError::without_span(format!("scenario CKB-VM execution failed: {error}")))?;
    let cycles = machine.machine.cycles();
    if exit_code == 0 {
        Ok(Observation::Passed { result: "()".to_string(), steps: None, cycles: Some(cycles), trace: Vec::new() })
    } else {
        let code = u64::try_from(exit_code)
            .map_err(|_| CompileError::without_span(format!("scenario CKB-VM returned negative exit code {exit_code}")))?;
        let runtime = CellScriptRuntimeError::from_code(code)
            .ok_or_else(|| CompileError::without_span(format!("scenario CKB-VM returned unregistered runtime code {code}")))?;
        Ok(Observation::RuntimeError { code, name: runtime.name().to_string(), steps: None, cycles: Some(cycles), trace: Vec::new() })
    }
}

#[cfg(not(feature = "vm-runner"))]
fn run_ckb_vm(_result: &CompileResult, _scenario: &Scenario) -> Result<Observation> {
    Err(CompileError::without_span("ckb-vm test backend is unavailable because the binary was built without vm-runner"))
}

fn validate_expectation(expectation: &ScenarioExpectation, observed: &Observation, step: &str) -> Result<()> {
    match (expectation.status.as_str(), observed) {
        ("pass", Observation::Passed { result, .. }) => {
            if expectation.result.as_ref().is_some_and(|expected| expected != result) {
                return Err(CompileError::without_span(format!(
                    "scenario step '{step}' result mismatch: expected {:?}, observed '{result}'",
                    expectation.result
                )));
            }
            Ok(())
        }
        ("runtime-error", Observation::RuntimeError { code, name, .. }) => {
            let expected = expectation.runtime_error.as_ref().expect("validated runtime error");
            if expected.code == *code && expected.name == *name {
                Ok(())
            } else {
                Err(CompileError::without_span(format!(
                    "scenario step '{step}' runtime error mismatch: expected {} ({}), observed {} ({})",
                    expected.code, expected.name, code, name
                )))
            }
        }
        (expected, observed) => Err(CompileError::without_span(format!(
            "scenario step '{step}' expected '{expected}' but observed {}",
            observation_status(observed)
        ))),
    }
}

fn observation_report(step: &ScenarioStep, observation: Observation) -> Value {
    match observation {
        Observation::Passed { result, steps, cycles, trace } => json!({
            "name": step.name,
            "status": "passed",
            "result": result,
            "runtime_error": null,
            "steps": steps,
            "cycles": cycles,
            "trace": trace,
            "transaction": transaction_shape_report(step)
        }),
        Observation::RuntimeError { code, name, steps, cycles, trace } => json!({
            "name": step.name,
            "status": "expected-runtime-error",
            "result": null,
            "runtime_error": {"code": code, "name": name},
            "steps": steps,
            "cycles": cycles,
            "trace": trace,
            "transaction": transaction_shape_report(step)
        }),
    }
}

fn transaction_shape_report(step: &ScenarioStep) -> Value {
    json!({
        "consumes": step.consumes,
        "outputs": step.outputs.iter().map(|cell| cell.name.as_str()).collect::<Vec<_>>(),
        "cell_deps": step.cell_deps.iter().map(|dep| dep.name.as_str()).collect::<Vec<_>>(),
        "header_deps": step.header_deps.iter().map(|dep| dep.name.as_str()).collect::<Vec<_>>(),
        "since_inputs": step.since.keys().collect::<Vec<_>>(),
        "witness_inputs": step.witnesses.iter().map(|witness| witness.input.as_str()).collect::<Vec<_>>()
    })
}

fn observation_status(observation: &Observation) -> &'static str {
    match observation {
        Observation::Passed { .. } => "pass",
        Observation::RuntimeError { .. } => "runtime-error",
    }
}

fn checker_report(result: &CompileResult) -> Result<cellscript_artifact_checker::CheckerReport> {
    let record = result
        .verified_lowering_record
        .as_ref()
        .ok_or_else(|| CompileError::without_span("scenario ELF has no verified lowering record"))?;
    let source_map =
        result.source_artifact_map.as_ref().ok_or_else(|| CompileError::without_span("scenario ELF has no source map"))?;
    let metadata = serde_json::to_value(&result.metadata)
        .map_err(|error| CompileError::without_span(format!("failed to encode scenario metadata: {error}")))?;
    cellscript_artifact_checker::check_bundle_values(
        &result.artifact_bytes,
        &metadata,
        record,
        source_map,
        &cellscript_artifact_checker::CheckerBudgets::default(),
    )
    .map_err(|error| CompileError::without_span(format!("scenario artifact checker rejected the build: {error}")))
}

fn coverage_report(result: &CompileResult, scenario: &Scenario, backend: TestBackend) -> Value {
    let Some(record) = result.verified_lowering_record.as_ref() else {
        return Value::Null;
    };
    let Some(source_map) = result.source_artifact_map.as_ref() else {
        return Value::Null;
    };
    let entry = &scenario.entry;
    let entry_record = record.entries.iter().find(|candidate| candidate.name == entry.name);
    let entry_id = entry_record.map(|entry| entry.id.as_str());
    let blocks = record
        .blocks
        .iter()
        .filter(|block| Some(block.owner_entry.as_str()) == entry_id)
        .map(|block| block.id.clone())
        .collect::<Vec<_>>();
    let entry_block = entry_record.map(|entry| entry.entry_block.clone());
    let intervals = source_map
        .intervals
        .iter()
        .filter(|interval| Some(interval.entry_id.as_str()) == entry_id)
        .map(|interval| {
            json!({
                "source_path": interval.source_path,
                "source_start": interval.source_start,
                "source_end": interval.source_end,
                "block_id": interval.block_id,
                "machine_range": interval.machine_range,
                "proof_ids": interval.proof_ids,
                "runtime_error_codes": interval.runtime_error_codes
            })
        })
        .collect::<Vec<_>>();
    let proofs = entry_record.map(|entry| entry.proof_ids.clone()).unwrap_or_default();
    let runtime_errors = record
        .runtime_error_exits
        .iter()
        .filter(|exit| blocks.contains(&exit.block_id))
        .map(|exit| json!({"code": exit.code, "name": exit.name, "block_id": exit.block_id}))
        .collect::<Vec<_>>();
    let observed_runtime_codes = scenario
        .steps
        .iter()
        .filter_map(|step| step.expectation.runtime_error.as_ref().map(|error| error.code as i32))
        .collect::<BTreeSet<_>>();
    let observed_runtime_errors = record
        .runtime_error_exits
        .iter()
        .filter(|exit| blocks.contains(&exit.block_id) && observed_runtime_codes.contains(&exit.code))
        .map(|exit| json!({"code": exit.code, "name": exit.name, "block_id": exit.block_id}))
        .collect::<Vec<_>>();
    let syscalls = record
        .syscall_sites
        .iter()
        .filter(|site| blocks.contains(&site.block_id))
        .map(|site| json!({"block_id": site.block_id, "address": site.address, "contract": site.contract}))
        .collect::<Vec<_>>();
    json!({
        "claim": "observed-entry-only;unexecuted-branches-not-claimed",
        "evidence_tier": backend.evidence_tier(),
        "entries": {"declared": [entry.name.clone()], "observed": [entry.name.clone()]},
        "lowering_blocks": {"declared": blocks, "observed": entry_block.into_iter().collect::<Vec<_>>()},
        "proof_plan_obligations": {"declared": proofs, "observed": []},
        "runtime_error_paths": {"declared": runtime_errors, "observed": observed_runtime_errors},
        "syscall_sites": {"declared": syscalls, "observed": []},
        "source_links": intervals
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script() -> ScenarioScript {
        ScenarioScript { code_hash: "11".repeat(32), hash_type: "data1".to_string(), args: String::new() }
    }

    fn cell(name: &str, prior: Option<&str>) -> ScenarioCell {
        ScenarioCell {
            name: name.to_string(),
            capacity: 100,
            data: String::new(),
            lock: script(),
            type_script: None,
            prior_output: prior.map(str::to_string),
        }
    }

    #[test]
    fn local_state_transition_rejects_reuse_of_consumed_cells() {
        let scenario = Scenario {
            schema: SCENARIO_SCHEMA.to_string(),
            name: "reuse".to_string(),
            source: "main.cell".to_string(),
            target_profile: "ckb".to_string(),
            entry: ScenarioEntry { kind: "action".to_string(), name: "main".to_string(), args: Vec::new() },
            initial_cells: vec![cell("c0", None)],
            steps: vec![
                ScenarioStep {
                    name: "first".to_string(),
                    consumes: vec!["c0".to_string()],
                    outputs: vec![cell("c1", Some("c0"))],
                    cell_deps: Vec::new(),
                    header_deps: Vec::new(),
                    since: BTreeMap::new(),
                    witnesses: Vec::new(),
                    expectation: ScenarioExpectation { status: "pass".to_string(), result: None, runtime_error: None },
                },
                ScenarioStep {
                    name: "second".to_string(),
                    consumes: vec!["c0".to_string()],
                    outputs: Vec::new(),
                    cell_deps: Vec::new(),
                    header_deps: Vec::new(),
                    since: BTreeMap::new(),
                    witnesses: Vec::new(),
                    expectation: ScenarioExpectation { status: "pass".to_string(), result: None, runtime_error: None },
                },
            ],
            limits: ScenarioLimits {
                max_steps: 100,
                max_cycles: 1_000_000,
                max_transaction_bytes: 1_000_000,
                minimum_cell_capacity: 1,
            },
            oracle: None,
        };
        assert!(validate_state_transitions(&scenario).unwrap_err().to_string().contains("missing, dead"));
    }

    #[test]
    fn unknown_scenario_fields_fail_closed() {
        let error = serde_json::from_str::<Scenario>(
            r#"{"schema":"cellscript-test-scenario-v1","name":"x","source":"x.cell","target_profile":"ckb","entry":{"kind":"action","name":"main","args":[]},"initial_cells":[],"steps":[],"limits":{"max_steps":1,"max_cycles":1,"max_transaction_bytes":1,"minimum_cell_capacity":1},"unknown":true}"#,
        )
        .unwrap_err();
        assert!(error.to_string().contains("unknown field"));
    }
}
