use std::process::Command;

use super::types::PackageSpec;

pub(super) fn build_install_cmd(spec: &PackageSpec) -> Option<Command> {
    match spec {
        PackageSpec::NpmGlobal { package } => {
            let mut cmd = Command::new("npm");
            cmd.args(["install", "-g", package]);
            Some(cmd)
        }
        PackageSpec::NpxInstaller { package, args } => {
            let mut cmd = Command::new("npx");
            cmd.args(["--yes", package]);
            cmd.args(args);
            Some(cmd)
        }
        PackageSpec::BunxInstaller { package, args } => build_bunx_cmd(package, args),
        PackageSpec::PythonTool { package } => build_python_install_cmd(package),
        PackageSpec::Manual { .. } => None,
    }
}

pub(super) fn build_update_cmd(spec: &PackageSpec) -> Option<Command> {
    match spec {
        PackageSpec::NpmGlobal { package } => {
            let mut cmd = Command::new("npm");
            cmd.args(["update", "-g", package]);
            Some(cmd)
        }
        PackageSpec::NpxInstaller { package, args } => {
            let mut cmd = Command::new("npx");
            cmd.args(["--yes", package]);
            cmd.args(args);
            Some(cmd)
        }
        PackageSpec::BunxInstaller { package, args } => build_bunx_cmd(package, args),
        PackageSpec::PythonTool { package } => build_python_update_cmd(package),
        PackageSpec::Manual { .. } => None,
    }
}

pub(super) fn build_uninstall_cmd(spec: &PackageSpec) -> Option<Command> {
    match spec {
        PackageSpec::NpmGlobal { package } => {
            let mut cmd = Command::new("npm");
            cmd.args(["uninstall", "-g", package]);
            Some(cmd)
        }
        PackageSpec::NpxInstaller { .. } | PackageSpec::BunxInstaller { .. } => None,
        PackageSpec::PythonTool { package } => build_python_uninstall_cmd(package),
        PackageSpec::Manual { .. } => None,
    }
}

fn build_bunx_cmd(package: &str, args: &[String]) -> Option<Command> {
    if which::which("bunx").is_ok() {
        let mut cmd = Command::new("bunx");
        cmd.arg(package);
        cmd.args(args);
        Some(cmd)
    } else {
        let mut cmd = Command::new("npx");
        cmd.args(["--yes", package]);
        cmd.args(args);
        Some(cmd)
    }
}

fn build_python_install_cmd(package: &str) -> Option<Command> {
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

fn build_python_update_cmd(package: &str) -> Option<Command> {
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

fn build_python_uninstall_cmd(package: &str) -> Option<Command> {
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
