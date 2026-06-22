use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use crate::isolation::paths::{
    create_private_dir_all, ensure_under_base, reject_existing_symlink_chain,
};
use crate::isolation::IsolationPaths;

pub(super) fn link_session_tree(
    source_dir: &Path,
    target_dir: &Path,
    paths: &IsolationPaths,
) -> Result<()> {
    if !source_dir.exists() {
        return Ok(());
    }
    if link_session_dir(source_dir, target_dir, paths)? {
        return Ok(());
    }
    let target_base = shared_target_base(target_dir, paths, "session link dir")?;
    create_private_dir_all(target_dir, target_base, "session link dir")?;
    link_session_tree_entries(source_dir, source_dir, target_dir, paths)
}

fn link_session_dir(source_dir: &Path, target_dir: &Path, paths: &IsolationPaths) -> Result<bool> {
    let metadata = fs::symlink_metadata(source_dir)
        .with_context(|| format!("inspect {}", source_dir.display()))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Ok(false);
    }
    let target_base = shared_target_base(target_dir, paths, "session link dir")?;
    ensure_under_base(target_dir, target_base, "session link dir")?;
    let parent = target_dir
        .parent()
        .with_context(|| format!("session link dir has no parent: {}", target_dir.display()))?;
    create_private_dir_all(parent, target_base, "session link dir")?;
    reject_existing_symlink_chain(parent, target_base, "session link dir")?;
    match fs::symlink_metadata(target_dir) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            if fs::read_link(target_dir)
                .with_context(|| format!("read link {}", target_dir.display()))?
                == source_dir
            {
                return Ok(true);
            }
            anyhow::bail!(
                "session link dir {} points at a different directory",
                target_dir.display(),
            );
        }
        Ok(_) => Ok(false),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            create_file_symlink(source_dir, target_dir).with_context(|| {
                format!(
                    "link session dir {} -> {}",
                    target_dir.display(),
                    source_dir.display()
                )
            })?;
            Ok(true)
        }
        Err(err) => Err(err).with_context(|| format!("inspect {}", target_dir.display())),
    }
}

fn link_session_tree_entries(
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
            link_session_tree_entries(root_source_dir, &source, target_dir, paths)?;
            continue;
        }
        if !metadata.is_file() || !is_session_file(&source) {
            continue;
        }
        let relative = source
            .strip_prefix(root_source_dir)
            .with_context(|| format!("resolve session path {}", source.display()))?;
        link_shared_file(&source, &target_dir.join(relative), paths, "session file")?;
    }
    Ok(())
}

pub(super) fn link_database_tree(
    source_dir: &Path,
    target_dir: &Path,
    paths: &IsolationPaths,
) -> Result<()> {
    if !source_dir.exists() {
        return Ok(());
    }
    let target_base = shared_target_base(target_dir, paths, "database link dir")?;
    create_private_dir_all(target_dir, target_base, "database link dir")?;
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

pub(super) fn link_shared_file(
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
    let target_base = shared_target_base(target, paths, label)?;
    ensure_under_base(target, target_base, label)?;
    let parent = target
        .parent()
        .with_context(|| format!("{label} has no parent: {}", target.display()))?;
    create_private_dir_all(parent, target_base, label)?;
    reject_existing_symlink_chain(parent, target_base, label)?;
    match fs::symlink_metadata(target) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            if fs::read_link(target).with_context(|| format!("read link {}", target.display()))?
                == source
            {
                return Ok(());
            }
            anyhow::bail!("{} {} points at a different file", label, target.display(),);
        }
        Ok(metadata) if can_replace_session_stub(target, &metadata, label)? => {
            fs::remove_file(target)
                .with_context(|| format!("remove stale {label} {}", target.display()))?;
            create_file_symlink(source, target).with_context(|| {
                format!("link {label} {} -> {}", target.display(), source.display())
            })?;
            Ok(())
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

pub(super) fn reject_source_symlink_chain(base: &Path, path: &Path, label: &str) -> Result<()> {
    let relative = path.strip_prefix(base).with_context(|| {
        format!(
            "{label} {} is outside source base {}",
            path.display(),
            base.display()
        )
    })?;
    let mut current = base.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err).with_context(|| format!("inspect {}", current.display())),
        };
        if metadata.file_type().is_symlink() {
            anyhow::bail!("{} {} traverses a source symlink", label, current.display());
        }
    }
    Ok(())
}

pub(super) fn is_session_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    is_database_file(path)
        || name.ends_with(".jsonl")
        || name.ends_with(".json")
        || name.ends_with(".txt")
        || name.ends_with(".log")
}

fn shared_target_base<'a>(
    target: &Path,
    paths: &'a IsolationPaths,
    label: &str,
) -> Result<&'a Path> {
    if target.starts_with(&paths.runtime_home) {
        return Ok(&paths.runtime_home);
    }
    if target.starts_with(&paths.home) {
        return Ok(&paths.home);
    }
    anyhow::bail!(
        "{} {} is outside isolation homes {} and {}",
        label,
        target.display(),
        paths.home.display(),
        paths.runtime_home.display()
    );
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

fn can_replace_session_stub(target: &Path, metadata: &fs::Metadata, label: &str) -> Result<bool> {
    if label != "session file" || !metadata.is_file() || metadata.len() > 2 {
        return Ok(false);
    }
    let contents = fs::read(target)
        .with_context(|| format!("read existing session file {}", target.display()))?;
    Ok(matches!(contents.as_slice(), b"" | b"{}" | b"[]"))
}

fn create_file_symlink(source: &Path, target: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(source, target)
}
