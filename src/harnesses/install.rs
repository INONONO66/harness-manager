use std::fs;
use std::process::Command;

use anyhow::{bail, Context};
use colored::Colorize;

use super::find_harness_spec;
use super::types::PackageSpec;
use crate::isolation::{self, hm_data_dir, IsolationPaths};
use crate::runtimes::types::IsolationSpec;

/// Build the install command for a package spec. Pure function for testing.
fn build_install_cmd(spec: &PackageSpec) -> Option<Command> {
    match spec {
        PackageSpec::NpmGlobal { package } => {
            let mut cmd = Command::new("npm");
            cmd.args(["install", "-g", package]);
            Some(cmd)
        }
        PackageSpec::PythonTool { package } => {
            // Prefer uv > pipx > pip
            if which::which("uv").is_ok() {
                let mut cmd = Command::new("uv");
                cmd.args(["tool", "install", package]);
                Some(cmd)
            } else if which::which("pipx").is_ok() {
                let mut cmd = Command::new("pipx");
                cmd.args(["install", package]);
                Some(cmd)
            } else if which::which("pip").is_ok() {
                let mut cmd = Command::new("pip");
                cmd.args(["install", "--user", package]);
                Some(cmd)
            } else if which::which("pip3").is_ok() {
                let mut cmd = Command::new("pip3");
                cmd.args(["install", "--user", package]);
                Some(cmd)
            } else {
                None
            }
        }
        PackageSpec::Manual { .. } => None,
    }
}

/// Build the update command for a package spec.
fn build_update_cmd(spec: &PackageSpec) -> Option<Command> {
    match spec {
        PackageSpec::NpmGlobal { package } => {
            let mut cmd = Command::new("npm");
            cmd.args(["update", "-g", package]);
            Some(cmd)
        }
        PackageSpec::PythonTool { package } => {
            if which::which("uv").is_ok() {
                let mut cmd = Command::new("uv");
                cmd.args(["tool", "upgrade", package]);
                Some(cmd)
            } else if which::which("pipx").is_ok() {
                let mut cmd = Command::new("pipx");
                cmd.args(["upgrade", package]);
                Some(cmd)
            } else if which::which("pip").is_ok() {
                let mut cmd = Command::new("pip");
                cmd.args(["install", "--user", "--upgrade", package]);
                Some(cmd)
            } else if which::which("pip3").is_ok() {
                let mut cmd = Command::new("pip3");
                cmd.args(["install", "--user", "--upgrade", package]);
                Some(cmd)
            } else {
                None
            }
        }
        PackageSpec::Manual { .. } => None,
    }
}

/// Build the uninstall command for a package spec.
fn build_uninstall_cmd(spec: &PackageSpec) -> Option<Command> {
    match spec {
        PackageSpec::NpmGlobal { package } => {
            let mut cmd = Command::new("npm");
            cmd.args(["uninstall", "-g", package]);
            Some(cmd)
        }
        PackageSpec::PythonTool { package } => {
            if which::which("uv").is_ok() {
                let mut cmd = Command::new("uv");
                cmd.args(["tool", "uninstall", package]);
                Some(cmd)
            } else if which::which("pipx").is_ok() {
                let mut cmd = Command::new("pipx");
                cmd.args(["uninstall", package]);
                Some(cmd)
            } else if which::which("pip").is_ok() {
                let mut cmd = Command::new("pip");
                cmd.args(["uninstall", "-y", package]);
                Some(cmd)
            } else {
                None
            }
        }
        PackageSpec::Manual { .. } => None,
    }
}

fn apply_isolation_env(cmd: &mut Command, iso: &IsolationSpec) {
    let paths = IsolationPaths::from_spec(iso);
    let _ = isolation::ensure_isolation_tree(iso, &paths);
    let _ = isolation::seed_files(iso, &paths);
    for (k, v) in isolation::build_isolation_env(iso, &paths) {
        cmd.env(k, v);
    }
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

pub fn install(id: &str) -> anyhow::Result<()> {
    let spec = find_harness_spec(id)
        .ok_or_else(|| anyhow::anyhow!("unknown harness: '{}'. Run `hm harness list` to see available harnesses.", id))?;

    eprintln!("{} harness '{}'...", "Installing".green().bold(), spec.display_name);

    if let PackageSpec::Manual { instructions } = &spec.package {
        eprintln!("{}", instructions);
        bail!("harness '{}' requires manual installation", id);
    }

    let mut cmd = build_install_cmd(&spec.package)
        .ok_or_else(|| anyhow::anyhow!("no suitable package manager found for harness '{}'", id))?;

    apply_isolation_env(&mut cmd, &spec.isolation);
    run_cmd(cmd, "install", id)?;

    eprintln!("{} harness '{}' installed successfully", "✓".green().bold(), spec.display_name);
    if let Some(caveat) = spec.isolation.caveat {
        eprintln!("{} {}", "Note:".yellow().bold(), caveat);
    }
    Ok(())
}

pub fn update(id: &str) -> anyhow::Result<()> {
    let spec = find_harness_spec(id)
        .ok_or_else(|| anyhow::anyhow!("unknown harness: '{}'. Run `hm harness list` to see available harnesses.", id))?;

    eprintln!("{} harness '{}'...", "Updating".green().bold(), spec.display_name);

    let mut cmd = build_update_cmd(&spec.package)
        .ok_or_else(|| anyhow::anyhow!("no suitable package manager found for harness '{}'", id))?;

    apply_isolation_env(&mut cmd, &spec.isolation);
    run_cmd(cmd, "update", id)?;

    eprintln!("{} harness '{}' updated successfully", "✓".green().bold(), spec.display_name);
    Ok(())
}

pub fn remove(id: &str, purge: bool) -> anyhow::Result<()> {
    let spec = find_harness_spec(id)
        .ok_or_else(|| anyhow::anyhow!("unknown harness: '{}'. Run `hm harness list` to see available harnesses.", id))?;

    eprintln!("{} harness '{}'...", "Removing".red().bold(), spec.display_name);

    if let Some(cmd) = build_uninstall_cmd(&spec.package) {
        // Best-effort uninstall — don't fail if the package wasn't installed
        let _ = run_cmd(cmd, "uninstall", id);
    }

    if purge {
        let harness_dir = hm_data_dir().join("runtimes").join(spec.isolation.subdir);
        if harness_dir.exists() {
            eprintln!("Purging isolation directory: {}", harness_dir.display());
            fs::remove_dir_all(&harness_dir)
                .with_context(|| format!("failed to purge {}", harness_dir.display()))?;
        }
    }

    eprintln!("{} harness '{}' removed", "✓".green().bold(), spec.display_name);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harnesses::types::PackageSpec;

    fn cmd_to_args(cmd: &Command) -> Vec<String> {
        let prog = cmd.get_program().to_string_lossy().to_string();
        let args: Vec<String> = cmd.get_args().map(|a| a.to_string_lossy().to_string()).collect();
        let mut result = vec![prog];
        result.extend(args);
        result
    }

    #[test]
    fn build_install_npm() {
        let spec = PackageSpec::NpmGlobal { package: "oh-my-codex" };
        let cmd = build_install_cmd(&spec).unwrap();
        let args = cmd_to_args(&cmd);
        assert_eq!(args, vec!["npm", "install", "-g", "oh-my-codex"]);
    }

    #[test]
    fn build_update_npm() {
        let spec = PackageSpec::NpmGlobal { package: "oh-my-codex" };
        let cmd = build_update_cmd(&spec).unwrap();
        let args = cmd_to_args(&cmd);
        assert_eq!(args, vec!["npm", "update", "-g", "oh-my-codex"]);
    }

    #[test]
    fn build_uninstall_npm() {
        let spec = PackageSpec::NpmGlobal { package: "oh-my-codex" };
        let cmd = build_uninstall_cmd(&spec).unwrap();
        let args = cmd_to_args(&cmd);
        assert_eq!(args, vec!["npm", "uninstall", "-g", "oh-my-codex"]);
    }

    #[test]
    fn build_install_manual_returns_none() {
        let spec = PackageSpec::Manual { instructions: "do it yourself" };
        assert!(build_install_cmd(&spec).is_none());
    }

    #[test]
    fn install_unknown_harness_errors() {
        let result = install("nonexistent-harness-xyz");
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("unknown harness"), "got: {}", msg);
    }
}
