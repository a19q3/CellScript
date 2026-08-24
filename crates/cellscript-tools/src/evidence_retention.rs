//! Bounded retention and content deduplication for local gate evidence.
//!
//! `target/` is a local working area, not the durable release archive. Keep a
//! small number of recent runs there, preserve exact report paths, and hardlink
//! immutable duplicate files across retained runs. CI/release automation that
//! needs longer retention must archive the named reports outside `target/` or
//! set `CELLSCRIPT_EVIDENCE_KEEP_RUNS=all`.

use std::collections::BTreeMap;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

const DEFAULT_KEEP_RUNS: usize = 3;
const MAX_CONFIGURED_KEEP_RUNS: usize = 128;
const DEDUP_MIN_BYTES: u64 = 64 * 1024;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct DedupStats {
    pub(crate) files: usize,
    pub(crate) bytes: u64,
}

pub(crate) fn keep_gate_workdirs() -> Result<bool> {
    let Some(value) = env::var_os("CELLSCRIPT_KEEP_GATE_WORKDIRS") else {
        return Ok(false);
    };
    match value.to_string_lossy().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" => Ok(true),
        "0" | "false" | "no" => Ok(false),
        value => bail!("CELLSCRIPT_KEEP_GATE_WORKDIRS must be 0/1, false/true, or no/yes; got {value:?}"),
    }
}

fn configured_keep_runs() -> Result<Option<usize>> {
    let Some(value) = env::var_os("CELLSCRIPT_EVIDENCE_KEEP_RUNS") else {
        return Ok(Some(DEFAULT_KEEP_RUNS));
    };
    let value = value.to_string_lossy();
    if value.eq_ignore_ascii_case("all") {
        return Ok(None);
    }
    let parsed = value
        .parse::<usize>()
        .with_context(|| format!("CELLSCRIPT_EVIDENCE_KEEP_RUNS must be 'all' or an integer from 1 to {MAX_CONFIGURED_KEEP_RUNS}"))?;
    if !(1..=MAX_CONFIGURED_KEEP_RUNS).contains(&parsed) {
        bail!("CELLSCRIPT_EVIDENCE_KEEP_RUNS must be 'all' or an integer from 1 to {MAX_CONFIGURED_KEEP_RUNS}");
    }
    Ok(Some(parsed))
}

fn file_name(path: &Path) -> Option<&str> {
    path.file_name().and_then(|name| name.to_str())
}

fn ensure_confined(root: &Path, path: &Path, label: &str) -> Result<()> {
    let canonical_root = fs::canonicalize(root).with_context(|| format!("failed to resolve evidence root {}", root.display()))?;
    let canonical_path = fs::canonicalize(path).with_context(|| format!("failed to resolve {label} {}", path.display()))?;
    if !canonical_path.starts_with(&canonical_root) {
        bail!("refusing to access {label} outside evidence root {}: {}", canonical_root.display(), canonical_path.display());
    }
    Ok(())
}

fn matching_children(root: &Path, parent: &Path, current: &Path, marker: &str, directories: bool) -> Result<Vec<PathBuf>> {
    if !parent.is_dir() {
        return Ok(Vec::new());
    }
    ensure_confined(root, parent, "evidence directory")?;
    ensure_confined(root, current, "current evidence path")?;
    let mut paths = Vec::new();
    for entry in fs::read_dir(parent).with_context(|| format!("failed to read evidence directory {}", parent.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path == current || !file_name(&path).is_some_and(|name| name.contains(marker)) {
            continue;
        }
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if (directories && metadata.is_dir()) || (!directories && metadata.is_file()) {
            paths.push(path);
        }
    }
    paths.sort_by(|left, right| file_name(right).cmp(&file_name(left)));
    Ok(paths)
}

pub(crate) fn prune_run_directories(root: &Path, parent: &Path, current: &Path, marker: &str) -> Result<Vec<PathBuf>> {
    let Some(keep) = configured_keep_runs()? else {
        return Ok(Vec::new());
    };
    prune_run_directories_with_limit(root, parent, current, marker, keep)
}

fn prune_run_directories_with_limit(root: &Path, parent: &Path, current: &Path, marker: &str, keep: usize) -> Result<Vec<PathBuf>> {
    let mut candidates = matching_children(root, parent, current, marker, true)?;
    let remove_from = keep.saturating_sub(1).min(candidates.len());
    let removed = candidates.split_off(remove_from);
    for path in &removed {
        fs::remove_dir_all(path).with_context(|| format!("failed to prune old evidence run {}", path.display()))?;
    }
    Ok(removed)
}

pub(crate) fn prune_report_files(root: &Path, parent: &Path, current: &Path, marker: &str) -> Result<Vec<PathBuf>> {
    let Some(keep) = configured_keep_runs()? else {
        return Ok(Vec::new());
    };
    let mut candidates = matching_children(root, parent, current, marker, false)?;
    let remove_from = keep.saturating_sub(1).min(candidates.len());
    let removed = candidates.split_off(remove_from);
    for path in &removed {
        fs::remove_file(path).with_context(|| format!("failed to prune old evidence report {}", path.display()))?;
    }
    Ok(removed)
}

fn collect_regular_files(path: &Path, minimum_bytes: u64, output: &mut Vec<PathBuf>) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Ok(());
    }
    if metadata.is_file() {
        if metadata.len() >= minimum_bytes {
            output.push(path.to_path_buf());
        }
        return Ok(());
    }
    if !metadata.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(path)? {
        collect_regular_files(&entry?.path(), minimum_bytes, output)?;
    }
    Ok(())
}

fn sha256(path: &Path) -> Result<[u8; 32]> {
    let mut file = File::open(path).with_context(|| format!("failed to hash {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 128 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().into())
}

fn replace_with_hardlink(source: &Path, destination: &Path, expected_hash: &[u8; 32], ordinal: usize) -> Result<bool> {
    let parent = destination.parent().context("evidence file has no parent")?;
    let temporary = parent.join(format!(".cellscript-dedup-{}-{ordinal}", std::process::id()));
    if temporary.exists() {
        fs::remove_file(&temporary)?;
    }
    if fs::hard_link(source, &temporary).is_err() {
        return Ok(false);
    }
    if sha256(&temporary)? != *expected_hash {
        fs::remove_file(&temporary)?;
        bail!("deduplication source changed while linking {}", source.display());
    }
    match fs::rename(&temporary, destination) {
        Ok(()) => Ok(true),
        Err(_) => {
            let _ = fs::remove_file(&temporary);
            Ok(false)
        }
    }
}

/// Hardlink immutable files in `current` to byte-identical files in sibling
/// evidence runs. Paths and bytes stay unchanged; unsupported filesystems
/// simply retain separate copies.
pub(crate) fn deduplicate_run(root: &Path, parent: &Path, current: &Path, marker: &str) -> Result<DedupStats> {
    let siblings = matching_children(root, parent, current, marker, true)?;
    if !current.is_dir() {
        return Ok(DedupStats::default());
    }

    let mut previous = BTreeMap::<(u64, [u8; 32]), PathBuf>::new();
    for sibling in siblings {
        let mut files = Vec::new();
        collect_regular_files(&sibling, DEDUP_MIN_BYTES, &mut files)?;
        files.sort();
        for path in files {
            let size = fs::metadata(&path)?.len();
            let digest = sha256(&path)?;
            previous.entry((size, digest)).or_insert(path);
        }
    }

    let mut current_files = Vec::new();
    collect_regular_files(current, DEDUP_MIN_BYTES, &mut current_files)?;
    current_files.sort();
    let mut stats = DedupStats::default();
    for (ordinal, path) in current_files.into_iter().enumerate() {
        let size = fs::metadata(&path)?.len();
        let digest = sha256(&path)?;
        let key = (size, digest);
        let Some(source) = previous.get(&key) else {
            previous.insert(key, path);
            continue;
        };
        if replace_with_hardlink(source, &path, &digest, ordinal)? {
            stats.files += 1;
            stats.bytes += size;
        }
    }
    Ok(stats)
}

pub(crate) fn remove_directory_if_present(root: &Path, path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            bail!("refusing to remove non-directory gate work path {}", path.display())
        }
        Ok(_) => {
            ensure_confined(root, path, "gate work directory")?;
            fs::remove_dir_all(path).with_context(|| format!("failed to remove gate work directory {}", path.display()))?;
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn write_latest_index(root: &Path, path: &Path, report_path: &Path, kind: &str, mode: &str, status: &str) -> Result<()> {
    let parent = path.parent().context("latest evidence index has no parent")?;
    ensure_confined(root, parent, "latest evidence index directory")?;
    ensure_confined(root, report_path, "evidence report")?;
    let report_bytes = fs::read(report_path).with_context(|| format!("failed to read {}", report_path.display()))?;
    let report_sha256 = hex::encode(Sha256::digest(&report_bytes));
    let relative = report_path.strip_prefix(parent).unwrap_or(report_path);
    let index = serde_json::json!({
        "schema": "cellscript-local-evidence-index-v1",
        "kind": kind,
        "mode": mode,
        "status": status,
        "report": {
            "path": relative.to_string_lossy().replace('\\', "/"),
            "sha256": report_sha256,
            "size_bytes": report_bytes.len(),
        },
    });
    let temporary = parent.join(format!(
        ".cellscript-latest-{}-{}-{}.tmp",
        std::process::id(),
        kind.replace(|character: char| !character.is_ascii_alphanumeric(), "_"),
        mode.replace(|character: char| !character.is_ascii_alphanumeric(), "_")
    ));
    let write_result = (|| -> Result<()> {
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .with_context(|| format!("failed to create temporary latest index {}", temporary.display()))?;
        serde_json::to_writer_pretty(&mut output, &index)?;
        output.write_all(b"\n")?;
        output.sync_all()?;
        fs::rename(&temporary, path).with_context(|| format!("failed to publish latest evidence index {}", path.display()))
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write_result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_root(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("cellscript-evidence-retention-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn pruning_keeps_current_and_newest_prior_runs() {
        let parent = test_root("prune");
        for name in ["run-1-quick", "run-2-quick", "run-3-quick", "run-4-ci"] {
            fs::create_dir(parent.join(name)).unwrap();
        }
        let current = parent.join("run-5-quick");
        fs::create_dir(&current).unwrap();

        let removed = prune_run_directories_with_limit(&parent, &parent, &current, "-quick", 3).unwrap();
        assert_eq!(removed.len(), 1);
        assert!(current.is_dir());
        assert!(parent.join("run-3-quick").is_dir());
        assert!(parent.join("run-2-quick").is_dir());
        assert!(!parent.join("run-1-quick").exists());
        assert!(parent.join("run-4-ci").is_dir());
        fs::remove_dir_all(parent).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn pruning_refuses_a_symlinked_evidence_root() {
        use std::os::unix::fs::symlink;

        let parent = test_root("confined-prune");
        let repository = parent.join("repository");
        let outside = parent.join("outside");
        fs::create_dir(&repository).unwrap();
        fs::create_dir(&outside).unwrap();
        fs::create_dir(outside.join("1-quick-old")).unwrap();
        fs::create_dir(outside.join("2-quick-current")).unwrap();
        symlink(&outside, repository.join("evidence")).unwrap();

        let error = prune_run_directories_with_limit(
            &repository,
            &repository.join("evidence"),
            &repository.join("evidence/2-quick-current"),
            "-quick-",
            1,
        )
        .unwrap_err();
        assert!(error.to_string().contains("outside evidence root"));
        assert!(outside.join("1-quick-old").is_dir());
        fs::remove_dir_all(parent).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn deduplication_preserves_paths_and_bytes() {
        use std::os::unix::fs::MetadataExt;

        let parent = test_root("dedup");
        let previous = parent.join("1-production");
        let current = parent.join("2-production");
        fs::create_dir(&previous).unwrap();
        fs::create_dir(&current).unwrap();
        let bytes = vec![0x5a; DEDUP_MIN_BYTES as usize];
        fs::write(previous.join("artifact.elf"), &bytes).unwrap();
        fs::write(current.join("artifact.elf"), &bytes).unwrap();

        let stats = deduplicate_run(&parent, &parent, &current, "-production").unwrap();
        assert_eq!(stats, DedupStats { files: 1, bytes: DEDUP_MIN_BYTES });
        assert_eq!(fs::read(current.join("artifact.elf")).unwrap(), bytes);
        assert_eq!(
            fs::metadata(previous.join("artifact.elf")).unwrap().ino(),
            fs::metadata(current.join("artifact.elf")).unwrap().ino()
        );
        fs::remove_dir_all(parent).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn deduplication_collapses_duplicates_inside_first_run() {
        use std::os::unix::fs::MetadataExt;

        let parent = test_root("dedup-first-run");
        let current = parent.join("1-bounded");
        fs::create_dir(&current).unwrap();
        let bytes = vec![0x33; DEDUP_MIN_BYTES as usize];
        fs::write(current.join("left.json"), &bytes).unwrap();
        fs::write(current.join("right.json"), &bytes).unwrap();

        let stats = deduplicate_run(&parent, &parent, &current, "-bounded").unwrap();
        assert_eq!(stats, DedupStats { files: 1, bytes: DEDUP_MIN_BYTES });
        assert_eq!(fs::metadata(current.join("left.json")).unwrap().ino(), fs::metadata(current.join("right.json")).unwrap().ino());
        fs::remove_dir_all(parent).unwrap();
    }
}
