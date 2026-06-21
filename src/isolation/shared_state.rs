use std::fs;
use std::path::Path;

use anyhow::{Context, Result};

use super::paths::validate_relative_path;
use super::IsolationPaths;
use crate::runtimes::manifest::SharedStatePlan;

mod cleanup;
mod glob;
mod link;

pub use cleanup::remove_runtime_shared_state_links;

use glob::expand_one_segment_glob;
use link::{
    is_session_file, link_database_tree, link_session_tree, link_shared_file,
    reject_source_symlink_chain,
};

const LEGACY_BUNDLED_AUTH_FILES: &[&str] = &[
    ".codex/auth.json",
    ".local/share/opencode/auth.json",
    ".claude/.credentials.json",
    ".pi/agent/auth.json",
    ".gjc/agent/auth-broker.token",
    ".grok/user-settings.json",
];

pub fn prepare_runtime_shared_state_with_auth(
    plan: Option<&SharedStatePlan>,
    paths: &IsolationPaths,
    share_auth_files: bool,
) -> Result<()> {
    let Some(home) = dirs::home_dir() else {
        return Ok(());
    };
    prepare_runtime_shared_state_from_home(plan, paths, &home, share_auth_files)
}

pub(crate) fn prepare_runtime_shared_state_from_home(
    plan: Option<&SharedStatePlan>,
    paths: &IsolationPaths,
    main_home: &Path,
    share_auth_files: bool,
) -> Result<()> {
    match plan {
        Some(plan) => prepare_shared_state_from_home(plan, paths, main_home, share_auth_files),
        None => Ok(()),
    }
}

pub(crate) fn prepare_shared_state_from_home(
    plan: &SharedStatePlan,
    paths: &IsolationPaths,
    main_home: &Path,
    share_auth_files: bool,
) -> Result<()> {
    for relative in &plan.database_dirs {
        validate_relative_path(relative, "shared_state.database_dirs")?;
        let source_dir = main_home.join(relative);
        reject_source_symlink_chain(main_home, &source_dir, "database link dir")?;
        let target_dir = paths.home.join(relative);
        link_database_tree(&source_dir, &target_dir, paths)?;
    }
    for relative in &plan.session_dirs {
        validate_relative_path(relative, "shared_state.session_dirs")?;
        let source_dir = main_home.join(relative);
        reject_source_symlink_chain(main_home, &source_dir, "session link dir")?;
        let target_dir = paths.home.join(relative);
        link_session_tree(&source_dir, &target_dir, paths)?;
    }
    for relative in &plan.session_files {
        validate_relative_path(relative, "shared_state.session_files")?;
        let source = main_home.join(relative);
        reject_source_symlink_chain(main_home, &source, "session file")?;
        let target = paths.home.join(relative);
        link_shared_file(&source, &target, paths, "session file")?;
    }
    for pattern in &plan.session_dir_globs {
        link_session_dir_glob(main_home, paths, pattern)?;
    }
    for pattern in &plan.session_file_globs {
        link_session_file_glob(main_home, paths, pattern)?;
    }
    remove_undeclared_legacy_auth_links(plan, paths)?;
    if !share_auth_files {
        remove_shared_auth_links(plan, paths)?;
        return Ok(());
    }
    for relative in &plan.auth_files {
        validate_relative_path(relative, "shared_state.auth_files")?;
        let source = main_home.join(relative);
        reject_source_symlink_chain(main_home, &source, "auth file")?;
        let target = paths.home.join(relative);
        link_shared_file(&source, &target, paths, "auth file")?;
    }
    Ok(())
}

pub(super) fn remove_undeclared_legacy_auth_links(
    plan: &SharedStatePlan,
    paths: &IsolationPaths,
) -> Result<()> {
    for relative in LEGACY_BUNDLED_AUTH_FILES {
        if plan.auth_files.iter().any(|declared| declared == relative) {
            continue;
        }
        remove_auth_link_if_present(paths, relative)?;
    }
    Ok(())
}

fn remove_shared_auth_links(plan: &SharedStatePlan, paths: &IsolationPaths) -> Result<()> {
    for relative in &plan.auth_files {
        validate_relative_path(relative, "shared_state.auth_files")?;
        remove_auth_link_if_present(paths, relative)?;
    }
    Ok(())
}

fn remove_auth_link_if_present(paths: &IsolationPaths, relative: &str) -> Result<()> {
    validate_relative_path(relative, "shared_state.auth_files")?;
    let target = paths.home.join(relative);
    match fs::symlink_metadata(&target) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            fs::remove_file(&target)
                .with_context(|| format!("remove shared auth link {}", target.display()))?;
        }
        Ok(_) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => return Err(err).with_context(|| format!("inspect {}", target.display())),
    }
    Ok(())
}

fn link_session_dir_glob(main_home: &Path, paths: &IsolationPaths, pattern: &str) -> Result<()> {
    let matches = expand_one_segment_glob(main_home, pattern, "shared_state.session_dir_globs")?;
    for relative in matches {
        let source_dir = main_home.join(&relative);
        if source_dir.is_dir() {
            reject_source_symlink_chain(main_home, &source_dir, "session link dir")?;
            let target_dir = paths.home.join(&relative);
            link_session_tree(&source_dir, &target_dir, paths)?;
        }
    }
    Ok(())
}

fn link_session_file_glob(main_home: &Path, paths: &IsolationPaths, pattern: &str) -> Result<()> {
    let matches = expand_one_segment_glob(main_home, pattern, "shared_state.session_file_globs")?;
    for relative in matches {
        let source = main_home.join(&relative);
        if source.is_file() && is_session_file(&source) {
            reject_source_symlink_chain(main_home, &source, "session file")?;
            let target = paths.home.join(&relative);
            link_shared_file(&source, &target, paths, "session file")?;
        }
    }
    Ok(())
}
