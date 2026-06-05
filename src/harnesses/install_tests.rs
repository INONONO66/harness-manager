use std::process::Command;

use super::{apply_isolation_env, install};
use crate::harnesses::package::{build_install_cmd, build_uninstall_cmd, build_update_cmd};
use crate::harnesses::registry::{HarnessRegistry, HarnessSource};
use crate::harnesses::types::PackageSpec;

fn test_runtimes() -> crate::runtimes::registry::RuntimeRegistry {
    crate::runtimes::registry::RuntimeRegistry::builtin_only().unwrap()
}

fn builtin_registry() -> HarnessRegistry {
    HarnessRegistry::builtin_only(&test_runtimes()).unwrap()
}

fn plugin_registry() -> HarnessRegistry {
    HarnessRegistry::from_sources(
        &[HarnessSource::manifest(
            "install-plugin.toml",
            r#"
schema_version = 1
id = "install-plugin"
display_name = "Install Plugin"
target_runtime = "Codex CLI"
detect_binaries = ["install-plugin-bin"]
launch_args = []

[package]
kind = "manual"
instructions = "manual"

[isolation]
spoof_home = true
home_subdirs = [".codex"]
static_envs = { CODEX_HOME = "{home}/.codex" }
seed_files = []
"#,
        )],
        &test_runtimes(),
    )
    .unwrap()
}

fn cmd_to_args(cmd: &Command) -> Vec<String> {
    let prog = cmd.get_program().to_string_lossy().to_string();
    let args: Vec<String> = cmd
        .get_args()
        .map(|a| a.to_string_lossy().to_string())
        .collect();
    let mut result = vec![prog];
    result.extend(args);
    result
}

#[test]
fn build_install_npm() {
    let spec = PackageSpec::NpmGlobal {
        package: "demo-package".to_string(),
    };
    let cmd = build_install_cmd(&spec).unwrap();
    let args = cmd_to_args(&cmd);
    assert_eq!(args, vec!["npm", "install", "-g", "demo-package"]);
}

#[test]
fn build_update_npm() {
    let spec = PackageSpec::NpmGlobal {
        package: "demo-package".to_string(),
    };
    let cmd = build_update_cmd(&spec).unwrap();
    let args = cmd_to_args(&cmd);
    assert_eq!(args, vec!["npm", "update", "-g", "demo-package"]);
}

#[test]
fn build_uninstall_npm() {
    let spec = PackageSpec::NpmGlobal {
        package: "demo-package".to_string(),
    };
    let cmd = build_uninstall_cmd(&spec).unwrap();
    let args = cmd_to_args(&cmd);
    assert_eq!(args, vec!["npm", "uninstall", "-g", "demo-package"]);
}

#[test]
fn build_install_manual_returns_none() {
    let spec = PackageSpec::Manual {
        instructions: "do it yourself".to_string(),
    };
    assert!(build_install_cmd(&spec).is_none());
}

#[test]
fn install_unknown_harness_errors() {
    let registry = builtin_registry();
    let result = install(&registry, "nonexistent-harness-xyz");
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("unknown harness"), "got: {}", msg);
}

#[test]
fn npx_installer_includes_manifest_args() {
    let spec = PackageSpec::NpxInstaller {
        package: "demo-installer".to_string(),
        args: vec!["install".to_string()],
    };

    let cmd = build_install_cmd(&spec).unwrap();

    assert_eq!(
        cmd_to_args(&cmd),
        vec!["npx", "--yes", "demo-installer", "install"]
    );
}

#[test]
fn bunx_installer_uses_bunx_or_npx_with_manifest_args() {
    let spec = PackageSpec::BunxInstaller {
        package: "demo-bun-installer".to_string(),
        args: vec!["install".to_string()],
    };

    let cmd = build_install_cmd(&spec).unwrap();
    let args = cmd_to_args(&cmd);

    assert!(
        args == vec!["bunx", "demo-bun-installer", "install"]
            || args == vec!["npx", "--yes", "demo-bun-installer", "install"],
        "got: {:?}",
        args
    );
}

#[test]
fn apply_isolation_env_strips_hostile_vars() {
    let mut cmd = Command::new("dummy");
    let registry = plugin_registry();
    let spec = registry.find("install-plugin").unwrap();
    let paths = crate::isolation::IsolationPaths::try_from_spec(&spec.isolation).unwrap();

    apply_isolation_env(&mut cmd, &spec.isolation, &paths).unwrap();

    let envs: Vec<(String, Option<String>)> = cmd
        .get_envs()
        .map(|(k, v)| {
            (
                k.to_string_lossy().to_string(),
                v.map(|value| value.to_string_lossy().to_string()),
            )
        })
        .collect();
    assert!(
        envs.iter().any(|(k, v)| k == "HOME"
            && v.as_ref()
                .is_some_and(|value| value.ends_with("/runtimes/install-plugin/home"))),
        "expected isolated HOME in command env: {:?}",
        envs
    );
    assert!(
        envs.iter().any(|(k, v)| k == "CODEX_HOME"
            && v.as_ref()
                .is_some_and(|value| value.ends_with("/runtimes/install-plugin/home/.codex"))),
        "expected isolated CODEX_HOME in command env: {:?}",
        envs
    );
    assert!(
        envs.iter().all(|(k, _)| k != "OPENAI_API_KEY"),
        "expected OPENAI_API_KEY to be absent: {:?}",
        envs
    );
    assert!(
        envs.iter().all(|(k, _)| k != "CODEX_ACCESS_TOKEN"),
        "expected CODEX_ACCESS_TOKEN to be absent: {:?}",
        envs
    );
    assert!(
        envs.iter().all(|(k, _)| k != "ANTHROPIC_API_KEY"),
        "expected ANTHROPIC_API_KEY to be absent: {:?}",
        envs
    );
}
