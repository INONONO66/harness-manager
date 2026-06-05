use std::process::Command;

use super::{apply_isolation_env, install};
use crate::harnesses::find_harness_spec;
use crate::harnesses::package::{build_install_cmd, build_uninstall_cmd, build_update_cmd};
use crate::harnesses::types::PackageSpec;

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
        package: "oh-my-codex",
    };
    let cmd = build_install_cmd(&spec).unwrap();
    let args = cmd_to_args(&cmd);
    assert_eq!(args, vec!["npm", "install", "-g", "oh-my-codex"]);
}

#[test]
fn build_update_npm() {
    let spec = PackageSpec::NpmGlobal {
        package: "oh-my-codex",
    };
    let cmd = build_update_cmd(&spec).unwrap();
    let args = cmd_to_args(&cmd);
    assert_eq!(args, vec!["npm", "update", "-g", "oh-my-codex"]);
}

#[test]
fn build_uninstall_npm() {
    let spec = PackageSpec::NpmGlobal {
        package: "oh-my-codex",
    };
    let cmd = build_uninstall_cmd(&spec).unwrap();
    let args = cmd_to_args(&cmd);
    assert_eq!(args, vec!["npm", "uninstall", "-g", "oh-my-codex"]);
}

#[test]
fn build_install_manual_returns_none() {
    let spec = PackageSpec::Manual {
        instructions: "do it yourself",
    };
    assert!(build_install_cmd(&spec).is_none());
}

#[test]
fn install_unknown_harness_errors() {
    let result = install("nonexistent-harness-xyz");
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("unknown harness"), "got: {}", msg);
}

#[test]
fn lazycodex_install_uses_npx_installer() {
    let spec = find_harness_spec("lazycodex").unwrap();

    let cmd = build_install_cmd(&spec.package).unwrap();

    assert_eq!(
        cmd_to_args(&cmd),
        vec!["npx", "--yes", "lazycodex-ai", "install"]
    );
}

#[test]
fn omo_install_uses_oh_my_openagent_installer() {
    let spec = find_harness_spec("omo").unwrap();

    let cmd = build_install_cmd(&spec.package).unwrap();
    let args = cmd_to_args(&cmd);

    assert!(
        args == vec!["bunx", "oh-my-openagent", "install"]
            || args == vec!["npx", "--yes", "oh-my-openagent", "install"],
        "got: {:?}",
        args
    );
}

#[test]
fn apply_isolation_env_strips_hostile_vars() {
    let mut cmd = Command::new("dummy");
    let spec = find_harness_spec("omx").unwrap();

    apply_isolation_env(&mut cmd, &spec.isolation).unwrap();

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
                .is_some_and(|value| value.ends_with("/runtimes/omx/home"))),
        "expected isolated HOME in command env: {:?}",
        envs
    );
    assert!(
        envs.iter().any(|(k, v)| k == "CODEX_HOME"
            && v.as_ref()
                .is_some_and(|value| value.ends_with("/runtimes/omx/home/.codex"))),
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
