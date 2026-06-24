use std::process::Command;

use super::{apply_isolation_env, apply_npm_isolated_env, install};
use crate::harnesses::manifest::PackageCommandTemplate;
use crate::harnesses::package::{
    build_install_cmd, build_uninstall_cmd_with_manager, build_update_cmd_with_manager,
};
use crate::harnesses::registry::{HarnessRegistry, HarnessSource};
use crate::harnesses::state::{read_package_manager, record_package_manager};
use crate::harnesses::types::PackageSpec;

#[path = "install_tests/npm_isolated.rs"]
mod npm_isolated;

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
        self_update: None,
    };
    let cmd = build_install_cmd(&spec).unwrap();
    let args = cmd_to_args(&cmd);
    assert_eq!(args, vec!["npm", "install", "-g", "demo-package"]);
}

#[test]
fn build_update_npm() {
    let spec = PackageSpec::NpmGlobal {
        package: "demo-package".to_string(),
        self_update: None,
    };
    let cmd = build_update_cmd_with_manager(&spec, None).unwrap();
    let args = cmd_to_args(&cmd);
    assert_eq!(args, vec!["npm", "update", "-g", "demo-package"]);
}

#[test]
fn build_uninstall_npm() {
    let spec = PackageSpec::NpmGlobal {
        package: "demo-package".to_string(),
        self_update: None,
    };
    let cmd = build_uninstall_cmd_with_manager(&spec, None).unwrap();
    let args = cmd_to_args(&cmd);
    assert_eq!(args, vec!["npm", "uninstall", "-g", "demo-package"]);
}

#[test]
fn build_install_manual_returns_none() {
    let spec = PackageSpec::Manual {
        instructions: "do it yourself".to_string(),
        self_update: None,
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
        self_update: None,
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
        self_update: None,
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
fn custom_backend_uses_manifest_argv_without_shell() {
    let spec = PackageSpec::Custom {
        install: PackageCommandTemplate {
            argv: vec![
                "installer".to_string(),
                "install".to_string(),
                "demo-package".to_string(),
            ],
        },
        update: Some(PackageCommandTemplate {
            argv: vec![
                "installer".to_string(),
                "upgrade".to_string(),
                "demo-package".to_string(),
            ],
        }),
        uninstall: Some(PackageCommandTemplate {
            argv: vec![
                "installer".to_string(),
                "remove".to_string(),
                "demo-package".to_string(),
            ],
        }),
        bin_subdir: Some(".custom/bin".to_string()),
        self_update: None,
    };

    assert_eq!(
        cmd_to_args(&build_install_cmd(&spec).unwrap()),
        vec!["installer", "install", "demo-package"]
    );
    assert_eq!(
        cmd_to_args(&build_update_cmd_with_manager(&spec, None).unwrap()),
        vec!["installer", "upgrade", "demo-package"]
    );
    assert_eq!(
        cmd_to_args(&build_uninstall_cmd_with_manager(&spec, None).unwrap()),
        vec!["installer", "remove", "demo-package"]
    );
}

#[test]
fn python_tool_update_and_uninstall_prefer_recorded_manager() {
    let spec = PackageSpec::PythonTool {
        package: "demo-package[extra]".to_string(),
        self_update: None,
    };

    assert_eq!(
        cmd_to_args(&build_update_cmd_with_manager(&spec, Some("pipx")).unwrap()),
        vec!["pipx", "upgrade", "demo-package"]
    );
    assert_eq!(
        cmd_to_args(&build_uninstall_cmd_with_manager(&spec, Some("pip3")).unwrap()),
        vec!["pip3", "uninstall", "-y", "demo-package"]
    );
}

#[test]
fn bunx_installer_update_prefers_recorded_npx_fallback() {
    let spec = PackageSpec::BunxInstaller {
        package: "demo-bun-installer".to_string(),
        args: vec!["install".to_string()],
        self_update: None,
    };

    assert_eq!(
        cmd_to_args(&build_update_cmd_with_manager(&spec, Some("npx")).unwrap()),
        vec!["npx", "--yes", "demo-bun-installer", "install"]
    );
}

#[test]
fn package_manager_state_round_trips_command_program() {
    let temp = tempfile::tempdir().unwrap();
    let paths = crate::isolation::IsolationPaths {
        base: temp.path().join("base"),
        home: temp.path().join("home"),
        state: temp.path().join("state"),
        tmp: temp.path().join("tmp"),
        runtime_base: temp.path().join("runtime-base"),
        runtime_home: temp.path().join("runtime-home"),
        runtime_state: temp.path().join("runtime-state"),
        runtime_logs: temp.path().join("runtime-logs"),
    };

    record_package_manager(&paths, "pipx").unwrap();

    assert_eq!(read_package_manager(&paths).as_deref(), Some("pipx"));
}

#[test]
fn apply_isolation_env_strips_hostile_vars() {
    let mut cmd = Command::new("dummy");
    let registry = plugin_registry();
    let spec = registry.find("install-plugin").unwrap();
    let paths = crate::isolation::IsolationPaths::try_from_spec(&spec.isolation).unwrap();

    apply_isolation_env(&mut cmd, None, &spec.isolation, &paths).unwrap();

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
