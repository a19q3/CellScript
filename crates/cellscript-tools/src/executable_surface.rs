use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};

const MARKDOWN_PATH: &str = "docs/generated/CELLSCRIPT_EXECUTABLE_SURFACE_MATRIX.md";
const JSON_PATH: &str = "docs/generated/cellscript-executable-surface.json";

pub fn run(root: &Path, write: bool) -> Result<()> {
    let markdown = cellscript::executable_surface::executable_surface_markdown();
    let json = cellscript::executable_surface::executable_surface_json();
    let markdown_path = root.join(MARKDOWN_PATH);
    let json_path = root.join(JSON_PATH);

    if write {
        if let Some(parent) = markdown_path.parent() {
            fs::create_dir_all(parent).with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::write(&markdown_path, markdown).with_context(|| format!("failed to write {}", markdown_path.display()))?;
        fs::write(&json_path, json).with_context(|| format!("failed to write {}", json_path.display()))?;
        println!("updated {MARKDOWN_PATH} and {JSON_PATH}");
        return Ok(());
    }

    let tracked_markdown = fs::read_to_string(&markdown_path)
        .with_context(|| format!("missing generated executable-surface matrix {}; rerun with --write", markdown_path.display()))?;
    let tracked_json = fs::read_to_string(&json_path)
        .with_context(|| format!("missing generated executable-surface matrix {}; rerun with --write", json_path.display()))?;
    let mut stale = Vec::new();
    if tracked_markdown != markdown {
        stale.push(MARKDOWN_PATH);
    }
    if tracked_json != json {
        stale.push(JSON_PATH);
    }
    if !stale.is_empty() {
        bail!("generated executable-surface matrix is stale: {}; rerun check-executable-surface --write", stale.join(", "));
    }
    Ok(())
}
