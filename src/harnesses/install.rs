use std::process::Command;

use anyhow::{bail, Context};
use colored::Colorize;

use super::find_harness_spec;
use super::package::{build_install_cmd, build_uninstall_cmd, build_update_cmd};
use super::types::{HarnessSpec, PackageSpec};
use crate::isolation::{self, IsolationPaths};
use crate::runtimes::types::IsolationSpec;

fn apply_isolation_env(cmd: &mut Command, iso: &IsolationSpec) -> anyhow::Result<()> {
    let paths = IsolationPaths::try_from_spec(iso)?;
    isolation::ensure_isolation_tree(iso, &paths)?;
    isolation::seed_files(iso, &paths)?;
    let inherited: std::collections::HashMap<String, String> = std::env::vars().collect();
    let env = isolation::build_sanitized_isolation_env(&inherited, iso, &paths);
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

fn harness_spec_or_err(id: &str) -> anyhow::Result<&'static HarnessSpec> {
    find_harness_spec(id).ok_or_else(|| {
        anyhow::anyhow!(
            "unknown harness: '{}'. Run `hm harness list` to see available harnesses.",
            id
        )
    })
}

pub fn install(id: &str) -> anyhow::Result<()> {
    let spec = harness_spec_or_err(id)?;

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

    apply_isolation_env(&mut cmd, &spec.isolation)?;
    run_cmd(cmd, "install", id)?;

    eprintln!(
        "{} harness '{}' installed successfully",
        "✓".green().bold(),
        spec.display_name
    );
    if let Some(caveat) = spec.isolation.caveat {
        eprintln!("{} {}", "Note:".yellow().bold(), caveat);
    }
    Ok(())
}

pub fn update(id: &str) -> anyhow::Result<()> {
    let spec = harness_spec_or_err(id)?;

    eprintln!(
        "{} harness '{}'...",
        "Updating".green().bold(),
        spec.display_name
    );

    let mut cmd = build_update_cmd(&spec.package)
        .ok_or_else(|| anyhow::anyhow!("no suitable package manager found for harness '{}'", id))?;

    apply_isolation_env(&mut cmd, &spec.isolation)?;
    run_cmd(cmd, "update", id)?;

    eprintln!(
        "{} harness '{}' updated successfully",
        "✓".green().bold(),
        spec.display_name
    );
    Ok(())
}

pub fn remove(id: &str, purge: bool) -> anyhow::Result<()> {
    let spec = harness_spec_or_err(id)?;

    eprintln!(
        "{} harness '{}'...",
        "Removing".red().bold(),
        spec.display_name
    );

    if let Some(cmd) = build_uninstall_cmd(&spec.package) {
        // Best-effort uninstall — don't fail if the package wasn't installed
        let mut cmd = cmd;
        apply_isolation_env(&mut cmd, &spec.isolation)?;
        let _ = run_cmd(cmd, "uninstall", id);
    }

    if purge {
        let paths = IsolationPaths::try_from_spec(&spec.isolation)?;
        eprintln!("Purging isolation directory: {}", paths.base.display());
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
