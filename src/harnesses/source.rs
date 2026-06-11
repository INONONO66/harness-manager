use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use toml_edit::{value, DocumentMut};

pub mod generated;

pub use generated::{add_package_harness, GeneratedHarnessPackage, GeneratedPackageKind};

use crate::harnesses::manifest::validation::validate_id;
use crate::harnesses::registry::{HarnessDiscoveryEnv, HarnessRegistry, HarnessSource};
use crate::runtimes::registry::RuntimeRegistry;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddedHarnessSource {
    alias: String,
    manifest_path: PathBuf,
}

impl AddedHarnessSource {
    pub fn new(alias: String, manifest_path: PathBuf) -> Self {
        Self {
            alias,
            manifest_path,
        }
    }

    pub fn alias(&self) -> &str {
        &self.alias
    }

    pub fn manifest_path(&self) -> &Path {
        &self.manifest_path
    }
}

pub fn add_harness_source(
    source: &str,
    alias: &str,
    data_home: &Path,
    runtimes: &RuntimeRegistry,
) -> Result<AddedHarnessSource> {
    validate_id("harness source alias", alias)?;
    ensure_alias_available(alias, data_home, runtimes)?;
    let plugin_dir = data_home.join("hm").join("plugins").join(alias);
    if plugin_dir.exists() {
        bail!(
            "harness alias '{}' already exists at {}",
            alias,
            plugin_dir.display()
        );
    }

    let plugins_root = plugin_dir
        .parent()
        .context("plugin directory should have a parent")?;
    fs::create_dir_all(plugins_root)
        .with_context(|| format!("create {}", plugins_root.display()))?;

    let temp_dir = plugins_root.join(format!(".{alias}.tmp-{}", std::process::id()));
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir)
            .with_context(|| format!("remove stale temp dir {}", temp_dir.display()))?;
    }

    let result = clone_and_prepare_source(source, alias, &temp_dir, &plugin_dir, runtimes);
    if result.is_err() {
        let _ = fs::remove_dir_all(&temp_dir);
    }
    result
}

pub fn data_home_from_process() -> Result<PathBuf> {
    if let Some(root) = std::env::var_os("XDG_DATA_HOME") {
        let path = PathBuf::from(root);
        if !path.as_os_str().is_empty() {
            return Ok(path);
        }
    }
    let home = dirs::home_dir().context("HOME is required when XDG_DATA_HOME is unset")?;
    Ok(home.join(".local").join("share"))
}

pub(crate) fn ensure_alias_available(
    alias: &str,
    data_home: &Path,
    runtimes: &RuntimeRegistry,
) -> Result<()> {
    let env = HarnessDiscoveryEnv {
        xdg_config_home: std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
        xdg_data_home: Some(data_home.to_path_buf()),
        home: dirs::home_dir(),
    };
    let registry = HarnessRegistry::load_from_env(&env, runtimes)?;
    if registry.find(alias).is_some() {
        bail!("harness alias '{}' is already registered", alias);
    }
    Ok(())
}

fn clone_and_prepare_source(
    source: &str,
    alias: &str,
    temp_dir: &Path,
    plugin_dir: &Path,
    runtimes: &RuntimeRegistry,
) -> Result<AddedHarnessSource> {
    let status = Command::new("git")
        .args(["clone", "--quiet", source])
        .arg(temp_dir)
        .status()
        .with_context(|| format!("failed to run git clone for harness source '{}'", source))?;
    if !status.success() {
        bail!(
            "git clone failed for harness source '{}' (exit code: {})",
            source,
            status.code().unwrap_or(-1)
        );
    }

    let manifest_path = temp_dir.join("harness.toml");
    if !manifest_path.is_file() {
        bail!(
            "harness source '{}' must contain harness.toml at the repository root",
            source
        );
    }
    rewrite_manifest_id(&manifest_path, alias)?;
    HarnessRegistry::from_sources(&[HarnessSource::File(manifest_path.clone())], runtimes)
        .with_context(|| {
            format!(
                "invalid harness source manifest {}",
                manifest_path.display()
            )
        })?;
    fs::rename(temp_dir, plugin_dir).with_context(|| {
        format!(
            "move harness source {} to {}",
            temp_dir.display(),
            plugin_dir.display()
        )
    })?;

    Ok(AddedHarnessSource::new(
        alias.to_string(),
        plugin_dir.join("harness.toml"),
    ))
}

fn rewrite_manifest_id(path: &Path, alias: &str) -> Result<()> {
    let input = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut document = input
        .parse::<DocumentMut>()
        .with_context(|| format!("parse {}", path.display()))?;
    document["id"] = value(alias);
    fs::write(path, document.to_string()).with_context(|| format!("write {}", path.display()))
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
