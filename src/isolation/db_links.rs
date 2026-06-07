use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::paths::{create_private_dir_all, ensure_under_base, reject_existing_symlink_chain};
use super::IsolationPaths;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DatabaseSharePolicy {
    Codex,
    OpenCode,
}

impl DatabaseSharePolicy {
    fn from_runtime_name(runtime_name: &str) -> Option<Self> {
        match runtime_name {
            "Codex CLI" => Some(Self::Codex),
            "OpenCode" => Some(Self::OpenCode),
            _ => None,
        }
    }

    fn dirs(self, main_home: &Path, paths: &IsolationPaths) -> (PathBuf, PathBuf) {
        match self {
            Self::Codex => (main_home.join(".codex"), paths.home.join(".codex")),
            Self::OpenCode => (
                main_home.join(".local/share/opencode"),
                paths.home.join(".local/share/opencode"),
            ),
        }
    }
}

pub fn link_main_runtime_databases(runtime_name: &str, paths: &IsolationPaths) -> Result<()> {
    let Some(home) = dirs::home_dir() else {
        return Ok(());
    };
    link_main_runtime_databases_from_home(runtime_name, paths, &home)
}

pub(crate) fn link_main_runtime_databases_from_home(
    runtime_name: &str,
    paths: &IsolationPaths,
    main_home: &Path,
) -> Result<()> {
    let Some(policy) = DatabaseSharePolicy::from_runtime_name(runtime_name) else {
        return Ok(());
    };
    let (source_dir, target_dir) = policy.dirs(main_home, paths);
    link_database_tree(&source_dir, &target_dir, paths)
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
        link_database_file(&source, &target_dir.join(relative), paths)?;
    }
    Ok(())
}

fn link_database_file(source: &Path, target: &Path, paths: &IsolationPaths) -> Result<()> {
    ensure_under_base(target, &paths.home, "database link")?;
    let parent = target
        .parent()
        .with_context(|| format!("database link has no parent: {}", target.display()))?;
    create_private_dir_all(parent, &paths.home, "database link parent")?;
    reject_existing_symlink_chain(parent, &paths.home, "database link parent")?;
    match fs::symlink_metadata(target) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            if fs::read_link(target).with_context(|| format!("read link {}", target.display()))?
                == source
            {
                return Ok(());
            }
            anyhow::bail!(
                "database link {} points at a different file",
                target.display()
            );
        }
        Ok(_) => anyhow::bail!(
            "database file {} already exists and is not a shared link",
            target.display()
        ),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            create_file_symlink(source, target).with_context(|| {
                format!("link database {} -> {}", target.display(), source.display())
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
