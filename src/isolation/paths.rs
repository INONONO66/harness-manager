use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

use super::IsolationPaths;

pub(super) fn validate_relative_path(value: &str, label: &str) -> Result<()> {
    let path = Path::new(value);
    if value.is_empty() || path.is_absolute() {
        anyhow::bail!("{} must be a non-empty relative path", label);
    }
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            _ => anyhow::bail!("{} contains an unsafe path component", label),
        }
    }
    Ok(())
}

fn reject_parent_components(path: &Path, label: &str) -> Result<()> {
    for component in path.components() {
        if matches!(component, Component::ParentDir) {
            anyhow::bail!("{} must not contain parent directory components", label);
        }
    }
    Ok(())
}

pub(super) fn ensure_under_base(path: &Path, base: &Path, label: &str) -> Result<()> {
    reject_parent_components(path, label)?;
    if !path.starts_with(base) {
        anyhow::bail!("{} must stay under {}", label, base.display());
    }
    Ok(())
}

pub(super) fn reject_existing_symlink_chain(path: &Path, base: &Path, label: &str) -> Result<()> {
    reject_symlink_if_exists(base, label)?;
    let relative = path
        .strip_prefix(base)
        .with_context(|| format!("{} must stay under {}", label, base.display()))?;
    let mut cursor = base.to_path_buf();
    for component in relative.components() {
        cursor.push(component);
        let Ok(metadata) = fs::symlink_metadata(&cursor) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            anyhow::bail!("{} must not traverse symlink {}", label, cursor.display());
        }
    }
    Ok(())
}

pub(super) fn isolation_root(paths: &IsolationPaths) -> Result<PathBuf> {
    let runtimes = paths
        .base
        .parent()
        .with_context(|| format!("isolation base has no parent: {}", paths.base.display()))?;
    runtimes
        .parent()
        .map(Path::to_path_buf)
        .with_context(|| format!("isolation base has no HM root: {}", paths.base.display()))
}

fn reject_symlink_if_exists(path: &Path, label: &str) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            anyhow::bail!("{} must not traverse symlink {}", label, path.display());
        }
        Ok(_) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err).with_context(|| format!("inspect {}", path.display())),
    }
}

pub(super) fn create_private_dir_all(path: &Path, trusted_base: &Path, label: &str) -> Result<()> {
    ensure_under_base(path, trusted_base, label)?;
    reject_existing_symlink_chain(path, trusted_base, label)?;
    fs::create_dir_all(path).with_context(|| format!("create {}", path.display()))?;
    reject_existing_symlink_chain(path, trusted_base, label)?;
    chmod_private_dir_chain(path, trusted_base)
}

pub(super) fn create_private_isolation_base(path: &Path, trusted_root: &Path) -> Result<()> {
    ensure_under_base(path, trusted_root, "isolation base")?;
    reject_existing_symlink_chain(path, trusted_root, "isolation base")?;
    fs::create_dir_all(path).with_context(|| format!("create {}", path.display()))?;
    reject_existing_symlink_chain(path, trusted_root, "isolation base")?;
    chmod_private_dir(path)
}

#[cfg(unix)]
fn chmod_private_dir(path: &Path) -> Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("chmod 700 {}", path.display()))
}

#[cfg(not(unix))]
fn chmod_private_dir(_path: &Path) -> Result<()> {
    Ok(())
}

fn chmod_private_dir_chain(path: &Path, trusted_base: &Path) -> Result<()> {
    let relative = path.strip_prefix(trusted_base).with_context(|| {
        format!(
            "{} must stay under {}",
            path.display(),
            trusted_base.display()
        )
    })?;
    let mut cursor = trusted_base.to_path_buf();
    for component in relative.components() {
        cursor.push(component);
        if cursor.is_dir() {
            chmod_private_dir(&cursor)?;
        }
    }
    Ok(())
}
