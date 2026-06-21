use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::glob::glob_segment_matches;
use super::remove_undeclared_legacy_auth_links;
use crate::isolation::paths::validate_relative_path;
use crate::isolation::IsolationPaths;
use crate::runtimes::manifest::SharedStatePlan;

pub fn remove_runtime_shared_state_links(
    plan: Option<&SharedStatePlan>,
    paths: &IsolationPaths,
) -> Result<()> {
    let Some(plan) = plan else {
        return Ok(());
    };
    for relative in &plan.database_dirs {
        validate_relative_path(relative, "shared_state.database_dirs")?;
        remove_symlinks_under(&paths.home.join(relative))?;
    }
    for relative in &plan.session_dirs {
        validate_relative_path(relative, "shared_state.session_dirs")?;
        remove_symlinks_under(&paths.home.join(relative))?;
    }
    for relative in &plan.session_files {
        validate_relative_path(relative, "shared_state.session_files")?;
        remove_symlink_file(&paths.home.join(relative))?;
    }
    for relative in &plan.auth_files {
        validate_relative_path(relative, "shared_state.auth_files")?;
        remove_symlink_file(&paths.home.join(relative))?;
    }
    remove_undeclared_legacy_auth_links(plan, paths)?;
    for pattern in &plan.session_dir_globs {
        remove_symlink_glob_matches(&paths.home, pattern, true, "shared_state.session_dir_globs")?;
    }
    for pattern in &plan.session_file_globs {
        remove_symlink_glob_matches(
            &paths.home,
            pattern,
            false,
            "shared_state.session_file_globs",
        )?;
    }
    Ok(())
}

fn remove_symlink_file(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            fs::remove_file(path)
                .with_context(|| format!("remove shared state link {}", path.display()))?;
        }
        Ok(_) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err).with_context(|| format!("inspect {}", path.display())),
    }
    Ok(())
}

fn remove_symlinks_under(path: &Path) -> Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err).with_context(|| format!("inspect {}", path.display())),
    };
    if metadata.file_type().is_symlink() {
        fs::remove_file(path)
            .with_context(|| format!("remove shared state link {}", path.display()))?;
        return Ok(());
    }
    if !metadata.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(path).with_context(|| format!("read {}", path.display()))? {
        remove_symlinks_under(&entry?.path())?;
    }
    Ok(())
}

fn remove_symlink_glob_matches(
    home: &Path,
    pattern: &str,
    remove_under_match: bool,
    label: &str,
) -> Result<()> {
    let components = parse_glob_components(pattern, label)?;
    remove_glob_components(home, &components, PathBuf::new(), remove_under_match)
}

fn parse_glob_components<'a>(pattern: &'a str, label: &str) -> Result<Vec<&'a str>> {
    if pattern.contains("**") || pattern.contains('\\') {
        anyhow::bail!("{label} contains an unsafe glob");
    }
    let components: Vec<&str> = pattern.split('/').collect();
    if components
        .iter()
        .any(|component| component.is_empty() || *component == "." || *component == "..")
    {
        anyhow::bail!("{label} contains an unsafe path component");
    }
    let wildcard_count = components
        .iter()
        .filter(|component| component.contains('*'))
        .count();
    if wildcard_count > 1 {
        anyhow::bail!("{label} may contain a wildcard in only one path segment");
    }
    Ok(components)
}

fn remove_glob_components(
    root: &Path,
    components: &[&str],
    relative_prefix: PathBuf,
    remove_under_match: bool,
) -> Result<()> {
    let Some((component, rest)) = components.split_first() else {
        let target = root.join(relative_prefix);
        if remove_under_match {
            remove_symlinks_under(&target)
        } else {
            remove_symlink_file(&target)
        }?;
        return Ok(());
    };
    if !component.contains('*') {
        return remove_glob_components(
            root,
            rest,
            relative_prefix.join(component),
            remove_under_match,
        );
    }
    let parent = root.join(&relative_prefix);
    let metadata = match fs::symlink_metadata(&parent) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err).with_context(|| format!("inspect {}", parent.display())),
    };
    if metadata.file_type().is_symlink() {
        return Ok(());
    }
    if !metadata.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(&parent).with_context(|| format!("read {}", parent.display()))? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if glob_segment_matches(component, name) {
            remove_glob_components(root, rest, relative_prefix.join(name), remove_under_match)?;
        }
    }
    Ok(())
}
