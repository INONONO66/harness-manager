use std::process::Command;

use anyhow::{bail, Context};
use colored::Colorize;

use super::package::{build_install_cmd, build_uninstall_cmd, build_update_cmd};
use super::registry::HarnessRegistry;
use super::types::{HarnessSpec, PackageSpec};
use crate::isolation::spec::IsolationRecipe;
use crate::isolation::{self, IsolationLockGuard, IsolationPaths};

fn apply_isolation_env(
    cmd: &mut Command,
    iso: &(impl IsolationRecipe + ?Sized),
    paths: &IsolationPaths,
) -> anyhow::Result<()> {
    isolation::ensure_isolation_tree(iso, paths)?;
    isolation::seed_files(iso, paths)?;
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

    if let PackageSpec::Manual { instructions } = &spec.package {
        eprintln!("{}", instructions);
        bail!("harness '{}' requires manual installation", id);
    }

    let mut cmd = build_install_cmd(&spec.package)
        .ok_or_else(|| anyhow::anyhow!("no suitable package manager found for harness '{}'", id))?;

    let paths = IsolationPaths::try_from_spec(&spec.isolation)?;
    let _lock = IsolationLockGuard::acquire(&paths)?;
    apply_isolation_env(&mut cmd, &spec.isolation, &paths)?;
    run_cmd(cmd, "install", id)?;

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

    let mut cmd = build_update_cmd(&spec.package)
        .ok_or_else(|| anyhow::anyhow!("no suitable package manager found for harness '{}'", id))?;

    let paths = IsolationPaths::try_from_spec(&spec.isolation)?;
    let _lock = IsolationLockGuard::acquire(&paths)?;
    apply_isolation_env(&mut cmd, &spec.isolation, &paths)?;
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

    if let Some(cmd) = build_uninstall_cmd(&spec.package) {
        // Best-effort uninstall — don't fail if the package wasn't installed
        let mut cmd = cmd;
        apply_isolation_env(&mut cmd, &spec.isolation, &paths)?;
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

#[cfg(test)]
#[path = "install_tests.rs"]
mod tests;
