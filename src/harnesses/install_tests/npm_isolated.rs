use std::process::Command;

use crate::harnesses::package::{build_install_cmd, build_uninstall_cmd};
use crate::harnesses::types::PackageSpec;

use super::{
    apply_isolation_env, apply_npm_isolated_env, builtin_registry, cmd_to_args, plugin_registry,
};

fn cmd_env_value(cmd: &Command, name: &str) -> Option<String> {
    cmd.get_envs().find_map(|(key, value)| {
        if key == name {
            value.map(|v| v.to_string_lossy().to_string())
        } else {
            None
        }
    })
}

#[test]
fn build_install_npm_isolated_uses_npm_install_g() {
    let spec = PackageSpec::NpmIsolated {
        package: "demo-package".to_string(),
        self_update: None,
    };
    let cmd = build_install_cmd(&spec).unwrap();
    let args = cmd_to_args(&cmd);
    assert_eq!(args, vec!["npm", "install", "-g", "demo-package"]);
}

#[test]
fn build_uninstall_npm_isolated_uses_npm_uninstall_g() {
    let spec = PackageSpec::NpmIsolated {
        package: "demo-package".to_string(),
        self_update: None,
    };
    let cmd = build_uninstall_cmd(&spec).unwrap();
    let args = cmd_to_args(&cmd);
    assert_eq!(args, vec!["npm", "uninstall", "-g", "demo-package"]);
}

#[test]
fn apply_npm_isolated_env_sets_prefix_and_cache_under_isolation() {
    let mut cmd = Command::new("dummy");
    let registry = plugin_registry();
    let spec = registry.find("install-plugin").unwrap();
    let paths = crate::isolation::IsolationPaths::try_from_spec(&spec.isolation).unwrap();

    apply_isolation_env(&mut cmd, None, &spec.isolation, &paths).unwrap();

    let isolated = PackageSpec::NpmIsolated {
        package: "demo".to_string(),
        self_update: None,
    };
    apply_npm_isolated_env(&mut cmd, &isolated, &paths);

    let envs: Vec<(String, Option<String>)> = cmd
        .get_envs()
        .map(|(k, v)| {
            (
                k.to_string_lossy().to_string(),
                v.map(|val| val.to_string_lossy().to_string()),
            )
        })
        .collect();

    assert!(
        envs.iter().any(|(k, v)| k == "NPM_CONFIG_PREFIX"
            && v.as_ref()
                .is_some_and(|val| val.ends_with("/runtimes/install-plugin/home/.npm"))),
        "expected NPM_CONFIG_PREFIX under isolation home: {:?}",
        envs
    );
    assert!(
        envs.iter().any(|(k, v)| k == "NPM_CONFIG_CACHE"
            && v.as_ref()
                .is_some_and(|val| val.ends_with("/runtimes/install-plugin/state/npm-cache"))),
        "expected NPM_CONFIG_CACHE under isolation state: {:?}",
        envs
    );
}

#[test]
fn omx_install_command_uses_isolated_codex_home_and_npm_prefix() {
    // Given: the real bundled omx spec and a temp isolation root.
    let mut cmd = build_install_cmd(&PackageSpec::NpmIsolated {
        package: "oh-my-codex".to_string(),
        self_update: None,
    })
    .unwrap();
    let registry = builtin_registry();
    let spec = registry.find("omx").unwrap();
    let temp = tempfile::tempdir().unwrap();
    let base = temp.path().join("hm/runtimes/omx");
    let paths = crate::isolation::IsolationPaths {
        base: base.clone(),
        home: base.join("home"),
        state: base.join("state"),
        tmp: base.join("tmp"),
        runtime_base: base.clone(),
        runtime_home: base.join("home"),
        runtime_state: base.join("state"),
        runtime_logs: base.join("state/logs"),
    };

    // When: hm prepares the package-manager command environment.
    apply_isolation_env(
        &mut cmd,
        spec.target_runtime_shared_state.as_ref(),
        &spec.isolation,
        &paths,
    )
    .unwrap();
    apply_npm_isolated_env(&mut cmd, &spec.package, &paths);

    // Then: npm, Codex, and HOME all point at the omx isolation tree.
    assert_eq!(
        cmd_to_args(&cmd),
        vec!["npm", "install", "-g", "oh-my-codex"]
    );
    assert_eq!(
        cmd_env_value(&cmd, "HOME").as_deref(),
        Some(paths.home.to_string_lossy().as_ref())
    );
    assert_eq!(
        cmd_env_value(&cmd, "CODEX_HOME").as_deref(),
        Some(paths.home.join(".codex").to_string_lossy().as_ref())
    );
    assert_eq!(
        cmd_env_value(&cmd, "NPM_CONFIG_PREFIX").as_deref(),
        Some(paths.home.join(".npm").to_string_lossy().as_ref())
    );
    assert_eq!(
        cmd_env_value(&cmd, "NPM_CONFIG_CACHE").as_deref(),
        Some(paths.state.join("npm-cache").to_string_lossy().as_ref())
    );
}

#[test]
fn apply_npm_isolated_env_is_no_op_for_npm_global() {
    let mut cmd = Command::new("dummy");
    let registry = plugin_registry();
    let spec = registry.find("install-plugin").unwrap();
    let paths = crate::isolation::IsolationPaths::try_from_spec(&spec.isolation).unwrap();

    let global = PackageSpec::NpmGlobal {
        package: "demo".to_string(),
        self_update: None,
    };
    apply_npm_isolated_env(&mut cmd, &global, &paths);

    let envs: Vec<String> = cmd
        .get_envs()
        .map(|(k, _)| k.to_string_lossy().to_string())
        .collect();

    assert!(
        !envs.iter().any(|k| k == "NPM_CONFIG_PREFIX"),
        "expected no NPM_CONFIG_PREFIX for NpmGlobal: {:?}",
        envs
    );
    assert!(
        !envs.iter().any(|k| k == "NPM_CONFIG_CACHE"),
        "expected no NPM_CONFIG_CACHE for NpmGlobal: {:?}",
        envs
    );
}
