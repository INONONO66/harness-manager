use std::fs;
use std::process::Command;

use anyhow::{bail, Context};
use colored::Colorize;

use super::package::{
    build_install_cmd, build_uninstall_cmd_with_manager, build_update_cmd_with_manager,
};
use super::registry::HarnessRegistry;
use super::types::{HarnessSpec, PackageSpec};
use crate::isolation::spec::IsolationRecipe;
use crate::isolation::{self, IsolationLockGuard, IsolationPaths};
use crate::runtimes::manifest::SharedStatePlan;

const PACKAGE_MANAGER_STATE_FILE: &str = "package-manager";

#[path = "install/git_worktree.rs"]
mod git_worktree;
#[path = "install/npm_isolated.rs"]
mod npm_isolated;

pub(super) use npm_isolated::apply_npm_isolated_env;

fn apply_isolation_env(
    cmd: &mut Command,
    target_runtime_shared_state: Option<&SharedStatePlan>,
    iso: &(impl IsolationRecipe + ?Sized),
    paths: &IsolationPaths,
) -> anyhow::Result<()> {
    isolation::ensure_isolation_tree(iso, paths)?;
    isolation::seed_files(iso, paths)?;
    isolation::remove_runtime_shared_state_links(target_runtime_shared_state, paths)?;
    let inherited: std::collections::HashMap<String, String> = std::env::vars().collect();
    let env = isolation::build_sanitized_isolation_env(&inherited, iso, paths);
    cmd.env_clear();
    for (k, v) in env {
        cmd.env(k, v);
    }
    Ok(())
}

fn run_cmd(mut cmd: Command, action: &str, id: &str) -> anyhow::Result<()> {
    let status = cmd
        .status()
        .with_context(|| format!("failed to run {} for harness '{}'", action, id))?;
    if !status.success() {
        bail!(
            "{} failed for harness '{}' (exit code: {})",
            action,
            id,
            status.code().unwrap_or(-1)
        );
    }
    Ok(())
}

fn run_cmd_with_env(
    mut cmd: Command,
    action: &str,
    id: &str,
    target_runtime_shared_state: Option<&SharedStatePlan>,
    iso: &(impl IsolationRecipe + ?Sized),
    paths: &IsolationPaths,
) -> anyhow::Result<()> {
    apply_isolation_env(&mut cmd, target_runtime_shared_state, iso, paths)?;
    run_cmd(cmd, action, id)
}

fn harness_spec_or_err<'a>(
    registry: &'a HarnessRegistry,
    id: &str,
) -> anyhow::Result<&'a HarnessSpec> {
    registry.find(id).ok_or_else(|| {
        let available = registry
            .specs()
            .iter()
            .map(|spec| {
                if spec.aliases.is_empty() {
                    spec.id.clone()
                } else {
                    format!("{} ({})", spec.id, spec.aliases.join(", "))
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        anyhow::anyhow!(
            "unknown harness: '{}'. Available harnesses: {}. Run `hm harness list` for status and install hints.",
            id,
            available
        )
    })
}

pub fn install(registry: &HarnessRegistry, id: &str) -> anyhow::Result<()> {
    let spec = harness_spec_or_err(registry, id)?;

    eprintln!(
        "{} harness '{}'...",
        "Installing".green().bold(),
        spec.display_name
    );

    if let PackageSpec::Manual { instructions, .. } = &spec.package {
        eprintln!("{}", instructions);
        bail!("harness '{}' requires manual installation", id);
    }

    let paths = IsolationPaths::try_from_spec(&spec.isolation)?;
    let _lock = IsolationLockGuard::acquire(&paths)?;
    if let PackageSpec::GitWorktree {
        repository, setup, ..
    } = &spec.package
    {
        git_worktree::prepare_package(repository, setup, "install", spec, &paths)?;
        record_package_manager(&paths, "git")?;
        eprintln!(
            "{} harness '{}' installed successfully",
            "✓".green().bold(),
            spec.display_name
        );
        if let Some(caveat) = spec.isolation.caveat() {
            eprintln!("{} {}", "Note:".yellow().bold(), caveat);
        }
        return Ok(());
    }

    let mut cmd = build_install_cmd(&spec.package)
        .ok_or_else(|| anyhow::anyhow!("no suitable package manager found for harness '{}'", id))?;

    apply_isolation_env(
        &mut cmd,
        spec.target_runtime_shared_state.as_ref(),
        &spec.isolation,
        &paths,
    )?;
    apply_npm_isolated_env(&mut cmd, &spec.package, &paths);
    let manager = command_program_name(&cmd);
    run_cmd(cmd, "install", id)?;
    if let Some(manager) = manager {
        record_package_manager(&paths, &manager)?;
    }

    eprintln!(
        "{} harness '{}' installed successfully",
        "✓".green().bold(),
        spec.display_name
    );
    if let Some(caveat) = spec.isolation.caveat() {
        eprintln!("{} {}", "Note:".yellow().bold(), caveat);
    }
    Ok(())
}

pub fn update(registry: &HarnessRegistry, id: &str) -> anyhow::Result<()> {
    let spec = harness_spec_or_err(registry, id)?;

    eprintln!(
        "{} harness '{}'...",
        "Updating".green().bold(),
        spec.display_name
    );

    let paths = IsolationPaths::try_from_spec(&spec.isolation)?;
    let preferred_manager = read_package_manager(&paths);
    let _lock = IsolationLockGuard::acquire(&paths)?;

    if let PackageSpec::GitWorktree {
        repository,
        setup,
        update,
        ..
    } = &spec.package
    {
        let template = update.as_ref().unwrap_or(setup);
        git_worktree::prepare_package(repository, template, "update", spec, &paths)?;
        return Ok(());
    }

    let mut cmd = build_update_cmd_with_manager(&spec.package, preferred_manager.as_deref())
        .ok_or_else(|| anyhow::anyhow!("no suitable package manager found for harness '{}'", id))?;

    apply_isolation_env(
        &mut cmd,
        spec.target_runtime_shared_state.as_ref(),
        &spec.isolation,
        &paths,
    )?;
    apply_npm_isolated_env(&mut cmd, &spec.package, &paths);
    run_cmd(cmd, "update", id)?;

    eprintln!(
        "{} harness '{}' updated successfully",
        "✓".green().bold(),
        spec.display_name
    );
    Ok(())
}

pub fn remove(registry: &HarnessRegistry, id: &str, purge: bool) -> anyhow::Result<()> {
    let spec = harness_spec_or_err(registry, id)?;

    eprintln!(
        "{} harness '{}'...",
        "Removing".red().bold(),
        spec.display_name
    );

    let paths = IsolationPaths::try_from_spec(&spec.isolation)?;
    let _lock = IsolationLockGuard::acquire(&paths)?;

    let preferred_manager = read_package_manager(&paths);
    if let Some(cmd) = build_uninstall_cmd_with_manager(&spec.package, preferred_manager.as_deref())
    {
        // Best-effort uninstall — don't fail if the package wasn't installed
        let mut cmd = cmd;
        apply_isolation_env(
            &mut cmd,
            spec.target_runtime_shared_state.as_ref(),
            &spec.isolation,
            &paths,
        )?;
        apply_npm_isolated_env(&mut cmd, &spec.package, &paths);
        let _ = run_cmd(cmd, "uninstall", id);
    }

    if purge {
        if paths.base.exists() {
            eprintln!("Purging isolation directory: {}", paths.base.display());
        }
        isolation::purge_isolation_tree(&paths)?;
    }

    eprintln!(
        "{} harness '{}' removed",
        "✓".green().bold(),
        spec.display_name
    );
    Ok(())
}

fn package_manager_state_path(paths: &IsolationPaths) -> std::path::PathBuf {
    paths.state.join(PACKAGE_MANAGER_STATE_FILE)
}

fn command_program_name(cmd: &Command) -> Option<String> {
    std::path::Path::new(cmd.get_program())
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
}

fn record_package_manager(paths: &IsolationPaths, manager: &str) -> anyhow::Result<()> {
    fs::create_dir_all(&paths.state)
        .with_context(|| format!("create {}", paths.state.display()))?;
    fs::write(package_manager_state_path(paths), format!("{manager}\n"))
        .with_context(|| "record package manager")
}

fn read_package_manager(paths: &IsolationPaths) -> Option<String> {
    let raw = fs::read_to_string(package_manager_state_path(paths)).ok()?;
    let manager = raw.trim();
    if manager.is_empty() {
        None
    } else {
        Some(manager.to_string())
    }
}

#[cfg(test)]
#[path = "install_tests.rs"]
mod tests;
