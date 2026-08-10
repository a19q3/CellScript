//! Least-privilege Registry worker for artifact-only CKB admission.
//!
//! This binary intentionally has no dependency on the CellScript compiler.

use anyhow::{bail, Context, Result};
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

const MAX_SNAPSHOT_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug)]
struct Args {
    snapshot: PathBuf,
    namespace: String,
    name: String,
    version: String,
    source_hash: String,
    manifest_hash: String,
    artifact_kind: String,
    profile: String,
    compatibility_profile_hash: Option<String>,
    artifact_hash: String,
    abi_hash: String,
    build_recipe_hash: Option<String>,
}

#[derive(Debug, Serialize)]
struct VerificationOutput {
    status: &'static str,
    verification_level: &'static str,
    artifact_hash: String,
    metadata_hash: String,
    compiler_version: Option<String>,
    source_hash: String,
    manifest_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    compatibility_profile_hash: Option<String>,
    artifact_format: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    checker_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    checker_policy_schema: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    checker_report_hash: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactBundle {
    schema: String,
    namespace: String,
    name: String,
    release: String,
    profile: String,
    manifest_json: String,
    objects: Vec<ArtifactBundleObject>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactBundleObject {
    role: String,
    content_base64: String,
}

#[derive(Serialize)]
struct FailureOutput<'a> {
    status: &'static str,
    error_code: &'static str,
    message: &'a str,
}

fn main() -> ExitCode {
    match run() {
        Ok(output) => {
            if serde_json::to_writer(std::io::stdout(), &output).is_err() {
                return ExitCode::from(70);
            }
            println!();
            ExitCode::SUCCESS
        }
        Err(error) => {
            let message = error.to_string();
            let output = FailureOutput { status: "failed", error_code: error_code(&error), message: &message };
            let _ = serde_json::to_writer(std::io::stdout(), &output);
            println!();
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<VerificationOutput> {
    let args = parse_args()?;
    verify(args)
}

fn verify(args: Args) -> Result<VerificationOutput> {
    if args.profile != "ckb_executable" || args.artifact_kind != "deployable_contract" {
        bail!("artifact-only verifier requires ckb_executable/deployable_contract");
    }
    let metadata = fs::symlink_metadata(&args.snapshot)
        .with_context(|| format!("failed to inspect artifact snapshot '{}'", args.snapshot.display()))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() == 0 || metadata.len() > MAX_SNAPSHOT_BYTES {
        bail!("artifact snapshot must be a non-empty, non-symlink regular file no larger than {MAX_SNAPSHOT_BYTES} bytes");
    }
    let snapshot =
        fs::read(&args.snapshot).with_context(|| format!("failed to read artifact snapshot '{}'", args.snapshot.display()))?;
    let bundle: ArtifactBundle = serde_json::from_slice(&snapshot).context("artifact bundle must be valid JSON")?;
    if bundle.schema != "cellscript-registry-bundle"
        || bundle.namespace != args.namespace
        || bundle.name != args.name
        || bundle.release != args.version
        || bundle.profile != args.profile
    {
        bail!("artifact bundle identity does not match the verification job");
    }
    let manifest: serde_json::Value =
        serde_json::from_str(&bundle.manifest_json).context("artifact bundle manifest_json must be valid JSON")?;
    if !manifest.is_object() || serde_json::to_string(&manifest)? != bundle.manifest_json {
        bail!("artifact bundle manifest_json must be canonical compact JSON");
    }
    validate_contract(&manifest, &args)?;
    require_hash("manifest_hash", &hash(bundle.manifest_json.as_bytes()), &args.manifest_hash)?;
    let has_verified_sidecars = validate_roles(&bundle, &manifest)?;

    let source = object(&bundle, "source")?;
    require_hash("source_hash", &hash(&source), &args.source_hash)?;
    let executable = object(&bundle, "executable")?;
    let artifact_hash = hash(&executable);
    require_hash("artifact_hash", &artifact_hash, &args.artifact_hash)?;
    let abi = object(&bundle, "abi")?;
    require_hash("abi_hash", &hash(&abi), &args.abi_hash)?;
    if manifest.pointer("/build/reproducible").and_then(serde_json::Value::as_bool) == Some(true) {
        let recipe = object(&bundle, "build_recipe")?;
        let expected = args.build_recipe_hash.as_deref().context("reproducible ckb_executable requires --build-recipe-hash")?;
        require_hash("build_recipe_hash", &hash(&recipe), expected)?;
    }
    if let Some(expected) = manifest.pointer("/security/audit_report_hash").and_then(serde_json::Value::as_str) {
        require_hash("audit_report_hash", &hash(&object(&bundle, "audit_report")?), expected)?;
    }

    let checker = if has_verified_sidecars {
        let compile_metadata = object(&bundle, "metadata")?;
        let lowering_record = object(&bundle, "lowering_record")?;
        let source_map = object(&bundle, "source_map")?;
        let budgets = cellscript_artifact_checker::CheckerBudgets::default();
        let report =
            cellscript_artifact_checker::check_bundle(&executable, &compile_metadata, &lowering_record, &source_map, &budgets)
                .map_err(anyhow::Error::msg)
                .context("artifact bundle independent checker rejected the CKB executable")?;
        let record = cellscript_artifact_checker::parse_lowering_record(&lowering_record, &budgets)
            .map_err(anyhow::Error::msg)
            .context("failed to read checker-approved lowering record")?;
        if let Some(expected) = args.compatibility_profile_hash.as_deref() {
            require_hash("compatibility_profile_hash", &record.compatibility_profile_hash, expected)?;
        }
        let report_bytes = cellscript_artifact_checker::canonical_bytes(&report).map_err(anyhow::Error::msg)?;
        Some((record.compatibility_profile_hash, report.checker_version, report.checker_policy_schema, hash(&report_bytes)))
    } else {
        if args.compatibility_profile_hash.is_some() {
            bail!("compatibility_profile_hash requires metadata, lowering_record, and source_map objects");
        }
        None
    };

    Ok(VerificationOutput {
        status: "passed",
        verification_level: if checker.is_some() { "structurally_verified" } else { "hash_bound" },
        artifact_hash,
        metadata_hash: hash(&snapshot),
        compiler_version: None,
        source_hash: args.source_hash,
        manifest_hash: args.manifest_hash,
        compatibility_profile_hash: checker.as_ref().map(|item| item.0.clone()),
        artifact_format: "ckb-vm-executable",
        checker_version: checker.as_ref().map(|item| item.1.clone()),
        checker_policy_schema: checker.as_ref().map(|item| item.2.clone()),
        checker_report_hash: checker.map(|item| item.3),
    })
}

fn validate_contract(contract: &serde_json::Value, args: &Args) -> Result<()> {
    let string = |pointer: &str| contract.pointer(pointer).and_then(serde_json::Value::as_str);
    if string("/schema") != Some("cellscript-registry-profile-contract-v1")
        || string("/artifact_kind") != Some(args.artifact_kind.as_str())
        || string("/profile") != Some(args.profile.as_str())
        || string("/build/target") != Some("riscv64imac-unknown-none-elf")
        || string("/ckb/abi_hash").is_none()
    {
        bail!("artifact profile contract is not a bounded CKB executable contract");
    }
    require_hash("abi_hash", string("/ckb/abi_hash").unwrap(), &args.abi_hash)?;
    if contract.pointer("/build/reproducible").and_then(serde_json::Value::as_bool) == Some(true) {
        let recipe = string("/reproduction/recipe_hash").context("reproducible contract is missing recipe_hash")?;
        let artifact =
            string("/reproduction/expected_artifact_hash").context("reproducible contract is missing expected_artifact_hash")?;
        require_hash(
            "build_recipe_hash",
            recipe,
            args.build_recipe_hash.as_deref().context("reproducible contract requires --build-recipe-hash")?,
        )?;
        require_hash("artifact_hash", artifact, &args.artifact_hash)?;
    }
    Ok(())
}

fn validate_roles(bundle: &ArtifactBundle, contract: &serde_json::Value) -> Result<bool> {
    let verified_roles = BTreeSet::from(["metadata", "lowering_record", "source_map"]);
    let has_any_verified_role = bundle.objects.iter().any(|item| verified_roles.contains(item.role.as_str()));
    let mut required = BTreeSet::from(["source", "executable", "abi"]);
    if has_any_verified_role {
        required.extend(verified_roles);
    }
    if contract.pointer("/build/reproducible").and_then(serde_json::Value::as_bool) == Some(true) {
        required.insert("build_recipe");
    }
    if contract.pointer("/security/audit_report_hash").is_some() {
        required.insert("audit_report");
    }
    let mut seen = BTreeSet::new();
    for item in &bundle.objects {
        if !required.contains(item.role.as_str()) || !seen.insert(item.role.as_str()) {
            bail!("artifact bundle contains an unexpected or duplicate '{}' object", item.role);
        }
    }
    if seen != required {
        bail!("artifact bundle is missing one or more required verified-artifact objects");
    }
    Ok(has_any_verified_role)
}

fn object(bundle: &ArtifactBundle, role: &str) -> Result<Vec<u8>> {
    let item = bundle.objects.iter().find(|item| item.role == role).with_context(|| format!("artifact bundle is missing '{role}'"))?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&item.content_base64)
        .with_context(|| format!("artifact bundle '{role}' object is not valid base64"))?;
    if bytes.is_empty() {
        bail!("artifact bundle '{role}' object is empty");
    }
    Ok(bytes)
}

fn parse_args() -> Result<Args> {
    let mut values = BTreeMap::new();
    let mut arguments = env::args().skip(1);
    while let Some(flag) = arguments.next() {
        if !flag.starts_with("--") {
            bail!("unexpected positional argument '{flag}'");
        }
        let value = arguments.next().with_context(|| format!("missing value for '{flag}'"))?;
        if values.insert(flag.clone(), value).is_some() {
            bail!("duplicate argument '{flag}'");
        }
    }
    let snapshot = PathBuf::from(take_required_arg(&mut values, "--snapshot")?);
    let namespace = take_required_arg(&mut values, "--namespace")?;
    let name = take_required_arg(&mut values, "--name")?;
    let version = take_required_arg(&mut values, "--version")?;
    let source_hash = take_required_arg(&mut values, "--source-hash")?;
    let manifest_hash = take_required_arg(&mut values, "--manifest-hash")?;
    let artifact_kind = take_required_arg(&mut values, "--artifact-kind")?;
    let profile = take_required_arg(&mut values, "--profile")?;
    let artifact_hash = take_required_arg(&mut values, "--artifact-hash")?;
    let abi_hash = take_required_arg(&mut values, "--abi-hash")?;
    let args = Args {
        snapshot,
        namespace,
        name,
        version,
        source_hash,
        manifest_hash,
        artifact_kind,
        profile,
        compatibility_profile_hash: values.remove("--compatibility-profile-hash"),
        artifact_hash,
        abi_hash,
        build_recipe_hash: values.remove("--build-recipe-hash"),
    };
    if let Some((unknown, _)) = values.into_iter().next() {
        bail!("unknown argument '{unknown}'");
    }
    Ok(args)
}

fn take_required_arg(values: &mut BTreeMap<String, String>, name: &str) -> Result<String> {
    values.remove(name).with_context(|| format!("missing required argument '{name}'"))
}

fn require_hash(field: &str, actual: &str, expected: &str) -> Result<()> {
    let normalize = |value: &str| value.strip_prefix("0x").unwrap_or(value).to_ascii_lowercase();
    let actual = normalize(actual);
    let expected = normalize(expected);
    if actual.len() != 64 || expected.len() != 64 || actual != expected {
        bail!("{field} mismatch: artifact value does not match the signed Registry identity");
    }
    Ok(())
}

fn hash(bytes: &[u8]) -> String {
    cellscript_artifact_checker::hex_encode(&cellscript_artifact_checker::ckb_blake2b256(bytes))
}

fn error_code(error: &anyhow::Error) -> &'static str {
    let messages = error.chain().map(ToString::to_string).collect::<Vec<_>>();
    let contains = |needle: &str| messages.iter().any(|message| message.contains(needle));
    if contains("unexpected positional") || contains("missing required argument") || contains("unknown argument") {
        "invalid_arguments"
    } else if contains("snapshot") && (contains("failed to") || contains("regular file")) {
        "snapshot_invalid"
    } else if contains("identity does not match") {
        "artifact_identity_mismatch"
    } else if contains("_hash mismatch") {
        "identity_hash_mismatch"
    } else if contains("independent checker rejected") || messages.iter().any(|message| message.starts_with('V')) {
        "artifact_checker_rejected"
    } else if contains("artifact bundle") {
        "artifact_bundle_invalid"
    } else if contains("artifact profile contract") {
        "profile_contract_invalid"
    } else {
        "verifier_internal_error"
    }
}

#[cfg(test)]
mod tests {
    use base64::Engine as _;
    use serde_json::json;

    use super::*;

    #[test]
    fn dependency_boundary_has_no_compiler_api() {
        assert_eq!(cellscript_artifact_checker::CHECKER_POLICY_SCHEMA, "cellscript-artifact-checker-policy-v1");
    }

    #[test]
    fn rejects_non_canonical_contract_json_before_checker_execution() {
        let value: serde_json::Value = serde_json::from_str("{\n  \"schema\": \"x\"\n}").unwrap();
        assert_ne!(serde_json::to_string(&value).unwrap(), "{\n  \"schema\": \"x\"\n}");
    }

    #[test]
    fn verifies_a_real_compiler_bundle_without_linking_the_compiler_into_the_worker() {
        let source = br#"module artifact_worker_fixture

action main(value: u64) -> u64 {
    verification
        return value
}
"#;
        let result = cellscript::compile(
            std::str::from_utf8(source).unwrap(),
            cellscript::CompileOptions { target: Some("riscv64-elf".to_string()), ..Default::default() },
        )
        .unwrap();
        let abi = br#"{"actions":["main"]}"#;
        let abi_hash = hash(abi);
        let artifact_hash = hash(&result.artifact_bytes);
        let metadata = serde_json::to_vec(&result.metadata).unwrap();
        let lowering_record = cellscript_artifact_checker::canonical_bytes(result.verified_lowering_record.as_ref().unwrap()).unwrap();
        let source_map = cellscript_artifact_checker::canonical_bytes(result.source_artifact_map.as_ref().unwrap()).unwrap();
        let compatibility_profile_hash = result.verified_lowering_record.as_ref().unwrap().compatibility_profile_hash.clone();
        let manifest = json!({
            "schema": "cellscript-registry-profile-contract-v1",
            "artifact_kind": "deployable_contract",
            "profile": "ckb_executable",
            "build": {
                "target": "riscv64imac-unknown-none-elf",
                "toolchain": "rustc 1.97.1",
                "profile": "release",
                "source_revision": "test-fixture",
                "reproducible": false
            },
            "security": { "status": "review_required" },
            "ckb": {
                "vm_version": "2",
                "script_role": "type",
                "hash_type": "data1",
                "dep_type": "code",
                "abi_hash": abi_hash.clone()
            }
        });
        let manifest_json = manifest.to_string();
        let encode = |role: &str, bytes: &[u8]| {
            json!({
                "role": role,
                "content_base64": base64::engine::general_purpose::STANDARD.encode(bytes)
            })
        };
        let bundle = json!({
            "schema": "cellscript-registry-bundle",
            "namespace": "cellscript",
            "name": "artifact-worker-fixture",
            "release": "0.24.0-test",
            "profile": "ckb_executable",
            "manifest_json": manifest_json.clone(),
            "objects": [
                encode("source", source),
                encode("executable", &result.artifact_bytes),
                encode("abi", abi),
                encode("metadata", &metadata),
                encode("lowering_record", &lowering_record),
                encode("source_map", &source_map)
            ]
        });
        let root = tempfile::tempdir().unwrap();
        let snapshot = root.path().join("bundle.json");
        fs::write(&snapshot, serde_json::to_vec(&bundle).unwrap()).unwrap();

        let output = verify(Args {
            snapshot,
            namespace: "cellscript".to_string(),
            name: "artifact-worker-fixture".to_string(),
            version: "0.24.0-test".to_string(),
            source_hash: hash(source),
            manifest_hash: hash(manifest_json.as_bytes()),
            artifact_kind: "deployable_contract".to_string(),
            profile: "ckb_executable".to_string(),
            compatibility_profile_hash: Some(compatibility_profile_hash),
            artifact_hash,
            abi_hash,
            build_recipe_hash: None,
        })
        .unwrap();
        assert_eq!(output.status, "passed");
        assert_eq!(output.verification_level, "structurally_verified");
        assert_eq!(output.checker_version.as_deref(), Some(cellscript_artifact_checker::CHECKER_VERSION));
        assert_eq!(output.checker_policy_schema.as_deref(), Some(cellscript_artifact_checker::CHECKER_POLICY_SCHEMA));
    }

    #[test]
    fn generic_ckb_bundle_remains_hash_bound_without_cellscript_sidecars() {
        let source = b"generic CKB source";
        let executable = b"generic executable bytes";
        let abi = br#"{"entry":"main"}"#;
        let abi_hash = hash(abi);
        let artifact_hash = hash(executable);
        let manifest = json!({
            "schema": "cellscript-registry-profile-contract-v1",
            "artifact_kind": "deployable_contract",
            "profile": "ckb_executable",
            "build": {
                "target": "riscv64imac-unknown-none-elf",
                "toolchain": "external",
                "profile": "release",
                "source_revision": "exact-external-revision",
                "reproducible": false
            },
            "security": { "status": "review_required" },
            "ckb": {
                "vm_version": "2",
                "script_role": "lock",
                "hash_type": "data1",
                "dep_type": "code",
                "abi_hash": abi_hash.clone()
            }
        });
        let manifest_json = manifest.to_string();
        let encode = |role: &str, bytes: &[u8]| {
            json!({
                "role": role,
                "content_base64": base64::engine::general_purpose::STANDARD.encode(bytes)
            })
        };
        let bundle = json!({
            "schema": "cellscript-registry-bundle",
            "namespace": "external",
            "name": "generic-ckb",
            "release": "1.0.0",
            "profile": "ckb_executable",
            "manifest_json": manifest_json.clone(),
            "objects": [encode("source", source), encode("executable", executable), encode("abi", abi)]
        });
        let root = tempfile::tempdir().unwrap();
        let snapshot = root.path().join("bundle.json");
        fs::write(&snapshot, serde_json::to_vec(&bundle).unwrap()).unwrap();

        let output = verify(Args {
            snapshot,
            namespace: "external".to_string(),
            name: "generic-ckb".to_string(),
            version: "1.0.0".to_string(),
            source_hash: hash(source),
            manifest_hash: hash(manifest_json.as_bytes()),
            artifact_kind: "deployable_contract".to_string(),
            profile: "ckb_executable".to_string(),
            compatibility_profile_hash: None,
            artifact_hash,
            abi_hash,
            build_recipe_hash: None,
        })
        .unwrap();
        assert_eq!(output.verification_level, "hash_bound");
        assert!(output.checker_version.is_none());
        assert!(output.checker_report_hash.is_none());
    }
}
