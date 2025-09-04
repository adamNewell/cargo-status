use crate::error::{CargoStatusError, Result};
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone)]
pub struct WorkspaceInfo {
    pub path: PathBuf,
    pub name: String,
    pub is_workspace: bool,
    pub member_count: usize,
}

pub fn find_cargo_projects(root: &Path, max_depth: usize) -> Result<Vec<WorkspaceInfo>> {
    let entries: Vec<_> = WalkDir::new(root)
        .max_depth(max_depth)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name() == "Cargo.toml")
        .map(|e| e.path().parent().unwrap().to_path_buf())
        .collect();

    entries
        .par_iter()
        .map(|path| analyze_workspace(path))
        .collect()
}

fn analyze_workspace(path: &Path) -> Result<WorkspaceInfo> {
    let cargo_toml_path = path.join("Cargo.toml");
    let contents = std::fs::read_to_string(&cargo_toml_path).map_err(|e| CargoStatusError::Io {
        context: format!("reading {}", cargo_toml_path.display()),
        source: e,
    })?;

    let value: toml::Value =
        toml::from_str(&contents).map_err(|e| CargoStatusError::CargoTomlParse { source: e })?;

    let name = value
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("unknown")
        .to_string();

    let is_workspace = value.get("workspace").is_some();

    let member_count = if is_workspace {
        value
            .get("workspace")
            .and_then(|w| w.get("members"))
            .and_then(|m| m.as_array())
            .map(|arr| arr.len())
            .unwrap_or(0)
    } else {
        0
    };

    Ok(WorkspaceInfo {
        path: path.to_path_buf(),
        name,
        is_workspace,
        member_count,
    })
}

pub fn scan_multiple_projects<F>(paths: &[PathBuf], callback: F) -> Vec<Result<WorkspaceInfo>>
where
    F: Fn(&Path) -> Result<WorkspaceInfo> + Sync,
{
    paths.par_iter().map(|path| callback(path)).collect()
}
