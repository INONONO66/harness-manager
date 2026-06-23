use std::fs;
use std::path::PathBuf;

use anyhow::Context;

use crate::isolation::IsolationPaths;

const PACKAGE_MANAGER_STATE_FILE: &str = "package-manager";

pub fn package_manager_state_path(paths: &IsolationPaths) -> PathBuf {
    paths.state.join(PACKAGE_MANAGER_STATE_FILE)
}

pub fn has_package_state(paths: &IsolationPaths) -> bool {
    read_package_manager(paths).is_some()
}

pub fn record_package_manager(paths: &IsolationPaths, manager: &str) -> anyhow::Result<()> {
    fs::create_dir_all(&paths.state)
        .with_context(|| format!("create {}", paths.state.display()))?;
    fs::write(package_manager_state_path(paths), format!("{manager}\n"))
        .with_context(|| "record package manager")
}

pub fn clear_package_state(paths: &IsolationPaths) -> anyhow::Result<()> {
    let path = package_manager_state_path(paths);
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
    }
    Ok(())
}

pub fn read_package_manager(paths: &IsolationPaths) -> Option<String> {
    let raw = fs::read_to_string(package_manager_state_path(paths)).ok()?;
    let manager = raw.trim();
    if manager.is_empty() {
        None
    } else {
        Some(manager.to_string())
    }
}
