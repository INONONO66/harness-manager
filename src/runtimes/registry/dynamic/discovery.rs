use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::RuntimeSource;

#[derive(Debug, Clone)]
pub struct RuntimeDiscoveryEnv {
    pub xdg_config_home: Option<PathBuf>,
    pub xdg_data_home: Option<PathBuf>,
    pub home: Option<PathBuf>,
}

impl RuntimeDiscoveryEnv {
    pub fn from_process() -> Self {
        Self {
            xdg_config_home: std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
            xdg_data_home: std::env::var_os("XDG_DATA_HOME").map(PathBuf::from),
            home: dirs::home_dir(),
        }
    }
}

pub(super) fn discover_config_sources(env: &RuntimeDiscoveryEnv) -> Result<Vec<RuntimeSource>> {
    if let Some(root) = &env.xdg_config_home {
        return discover_runtime_dir(&root.join("hm").join("runtimes.d"));
    }
    let Some(home) = &env.home else {
        return Ok(Vec::new());
    };
    discover_runtime_dir(&home.join(".config").join("hm").join("runtimes.d"))
}

pub(super) fn discover_data_sources(env: &RuntimeDiscoveryEnv) -> Result<Vec<RuntimeSource>> {
    let root = env.xdg_data_home.clone().or_else(|| {
        env.home
            .as_ref()
            .map(|home| home.join(".local").join("share"))
    });
    let Some(root) = root else {
        return Ok(Vec::new());
    };

    let mut sources = discover_runtime_dir(&root.join("hm").join("runtimes.d"))?;
    sources.extend(discover_plugin_sources(&root.join("hm").join("plugins"))?);
    Ok(sources)
}

fn discover_runtime_dir(dir: &Path) -> Result<Vec<RuntimeSource>> {
    let mut paths = manifest_files_in_dir(dir)?;
    paths.sort();
    Ok(paths.into_iter().map(RuntimeSource::File).collect())
}

fn discover_plugin_sources(dir: &Path) -> Result<Vec<RuntimeSource>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let canonical_root = dir
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", dir.display()))?;
    let mut paths = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        if entry.file_type()?.is_symlink() {
            anyhow::bail!(
                "plugin runtime path must not traverse symlink: {}",
                entry.path().display()
            );
        }
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path().join("runtime.toml");
        if path.exists() {
            reject_symlink_escape(&canonical_root, &path)?;
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths.into_iter().map(RuntimeSource::File).collect())
}

fn manifest_files_in_dir(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let canonical_root = dir
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", dir.display()))?;
    let mut paths = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("toml") {
            continue;
        }
        reject_symlink_escape(&canonical_root, &path)?;
        paths.push(path);
    }
    Ok(paths)
}

fn reject_symlink_escape(root: &Path, path: &Path) -> Result<()> {
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        anyhow::bail!("runtime manifest must not be a symlink: {}", path.display());
    }
    let canonical = path
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", path.display()))?;
    if !canonical.starts_with(root) {
        anyhow::bail!(
            "runtime manifest path escapes discovery root: {}",
            path.display()
        );
    }
    Ok(())
}
