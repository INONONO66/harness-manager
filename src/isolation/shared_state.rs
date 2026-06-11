use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use super::paths::{
    create_private_dir_all, ensure_under_base, reject_existing_symlink_chain,
    validate_relative_path,
};
use super::IsolationPaths;
use crate::runtimes::manifest::SharedStatePlan;

pub fn prepare_runtime_shared_state(
    plan: Option<&SharedStatePlan>,
    paths: &IsolationPaths,
) -> Result<()> {
    let Some(home) = dirs::home_dir() else {
        return Ok(());
    };
    prepare_runtime_shared_state_from_home(plan, paths, &home)
}

pub(crate) fn prepare_runtime_shared_state_from_home(
    plan: Option<&SharedStatePlan>,
    paths: &IsolationPaths,
    main_home: &Path,
) -> Result<()> {
    match plan {
        Some(plan) => prepare_shared_state_from_home(plan, paths, main_home),
        None => Ok(()),
    }
}

pub(crate) fn prepare_shared_state_from_home(
    plan: &SharedStatePlan,
    paths: &IsolationPaths,
    main_home: &Path,
) -> Result<()> {
    for relative in &plan.database_dirs {
        validate_relative_path(relative, "shared_state.database_dirs")?;
        let source_dir = main_home.join(relative);
        let target_dir = paths.home.join(relative);
        link_database_tree(&source_dir, &target_dir, paths)?;
    }
    for relative in &plan.auth_files {
        validate_relative_path(relative, "shared_state.auth_files")?;
        let source = main_home.join(relative);
        let target = paths.home.join(relative);
        link_shared_file(&source, &target, paths, "auth file")?;
    }
    Ok(())
}

fn link_database_tree(source_dir: &Path, target_dir: &Path, paths: &IsolationPaths) -> Result<()> {
    if !source_dir.exists() {
        return Ok(());
    }
    create_private_dir_all(target_dir, &paths.home, "database link dir")?;
    link_database_tree_entries(source_dir, source_dir, target_dir, paths)
}

fn link_database_tree_entries(
    root_source_dir: &Path,
    current_source_dir: &Path,
    target_dir: &Path,
    paths: &IsolationPaths,
) -> Result<()> {
    for entry in fs::read_dir(current_source_dir)
        .with_context(|| format!("read {}", current_source_dir.display()))?
    {
        let entry = entry?;
        let source = entry.path();
        let metadata = fs::symlink_metadata(&source)
            .with_context(|| format!("inspect {}", source.display()))?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            link_database_tree_entries(root_source_dir, &source, target_dir, paths)?;
            continue;
        }
        if !metadata.is_file() || !is_database_file(&source) {
            continue;
        }
        let relative = source
            .strip_prefix(root_source_dir)
            .with_context(|| format!("resolve database path {}", source.display()))?;
        link_shared_file(&source, &target_dir.join(relative), paths, "database file")?;
    }
    Ok(())
}

fn link_shared_file(
    source: &Path,
    target: &Path,
    paths: &IsolationPaths,
    label: &str,
) -> Result<()> {
    if !source.exists() {
        return Ok(());
    }
    let metadata =
        fs::symlink_metadata(source).with_context(|| format!("inspect {}", source.display()))?;
    if !metadata.is_file() {
        return Ok(());
    }
    ensure_under_base(target, &paths.home, label)?;
    let parent = target
        .parent()
        .with_context(|| format!("{label} has no parent: {}", target.display()))?;
    create_private_dir_all(parent, &paths.home, label)?;
    reject_existing_symlink_chain(parent, &paths.home, label)?;
    match fs::symlink_metadata(target) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            if fs::read_link(target).with_context(|| format!("read link {}", target.display()))?
                == source
            {
                return Ok(());
            }
            anyhow::bail!("{} {} points at a different file", label, target.display(),);
        }
        Ok(_) => anyhow::bail!(
            "{} {} already exists and is not a shared link",
            label,
            target.display(),
        ),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            create_file_symlink(source, target).with_context(|| {
                format!("link {label} {} -> {}", target.display(), source.display())
            })?;
            Ok(())
        }
        Err(err) => Err(err).with_context(|| format!("inspect {}", target.display())),
    }
}

fn is_database_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    name.ends_with(".sqlite")
        || name.ends_with(".sqlite-wal")
        || name.ends_with(".sqlite-shm")
        || name.ends_with(".db")
        || name.ends_with(".db-wal")
        || name.ends_with(".db-shm")
}

fn create_file_symlink(source: &Path, target: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(source, target)
}
