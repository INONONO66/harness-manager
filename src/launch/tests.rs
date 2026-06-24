use super::effective_profile_name;
use super::target::{build_launch_env, resolve_target, runtime_isolation_plan, LaunchTarget};
use crate::config::HmConfig;
use crate::harnesses::registry::HarnessRegistry;
use crate::isolation;
use crate::runtimes::registry::RuntimeRegistry;
use std::collections::HashMap;

fn test_runtimes() -> RuntimeRegistry {
    RuntimeRegistry::builtin_only().unwrap()
}

fn builtin_harnesses() -> HarnessRegistry {
    HarnessRegistry::builtin_only(&test_runtimes()).unwrap()
}

fn plugin_registry() -> HarnessRegistry {
    use crate::harnesses::types::{HarnessSpec, PackageSpec};
    use crate::isolation::spec::IsolationPlan;
    let spec = HarnessSpec {
        id: "launch-plugin".to_string(),
        aliases: vec![],
        display_name: "Launch Plugin".to_string(),
        target_runtime: "Codex CLI".to_string(),
        target_runtime_shared_state: None,
        package: PackageSpec::Manual {
            instructions: "manual".to_string(),
            self_update: None,
        },
        detect_binaries: vec!["launch-plugin-bin".to_string()],
        launch_binary: Some("plugin-wrapper".to_string()),
        launch_args: vec!["--plugin-mode".to_string()],
        isolation: IsolationPlan {
            subdir: "launch-plugin".to_string(),
            runtime_subdir: "launch-plugin".to_string(),
            home_subdirs: vec![".codex".to_string()],
            static_envs: vec![("CODEX_HOME".to_string(), "{home}/.codex".to_string())],
            seed_files: vec![],
            caveat: None,
        },
    };
    HarnessRegistry::from_specs(&test_runtimes(), vec![spec]).unwrap()
}

#[test]
fn resolve_target_runtime() {
    let runtimes = test_runtimes();
    let harnesses = builtin_harnesses();
    match resolve_target("codex", &runtimes, &harnesses).unwrap() {
        LaunchTarget::Runtime(rt) => assert_eq!(rt.name, "Codex CLI"),
        _ => panic!("expected Runtime"),
    }
}

#[test]
fn resolve_target_runtime_by_name() {
    let runtimes = test_runtimes();
    let harnesses = builtin_harnesses();
    match resolve_target("Codex CLI", &runtimes, &harnesses).unwrap() {
        LaunchTarget::Runtime(rt) => assert_eq!(rt.name, "Codex CLI"),
        _ => panic!("expected Runtime"),
    }
}

#[test]
fn resolve_target_harness() {
    let runtimes = test_runtimes();
    let harnesses = plugin_registry();
    match resolve_target("launch-plugin", &runtimes, &harnesses).unwrap() {
        LaunchTarget::Harness { harness, runtime } => {
            assert_eq!(harness.id, "launch-plugin");
            assert_eq!(runtime.name, "Codex CLI");
        }
        _ => panic!("expected Harness"),
    }
}

#[test]
fn resolve_target_harness_case_insensitive() {
    let runtimes = test_runtimes();
    let harnesses = plugin_registry();
    match resolve_target("LAUNCH-PLUGIN", &runtimes, &harnesses).unwrap() {
        LaunchTarget::Harness { harness, .. } => assert_eq!(harness.id, "launch-plugin"),
        _ => panic!("expected Harness"),
    }
}

#[test]
fn resolve_target_unknown() {
    let runtimes = test_runtimes();
    let harnesses = builtin_harnesses();
    assert!(resolve_target("nonexistent-xyz", &runtimes, &harnesses).is_err());
}

#[test]
fn resolve_target_plugin_harness_has_wrapper() {
    let runtimes = test_runtimes();
    let harnesses = plugin_registry();
    match resolve_target("launch-plugin", &runtimes, &harnesses).unwrap() {
        LaunchTarget::Harness { harness, .. } => {
            assert_eq!(harness.launch_binary.as_deref(), Some("plugin-wrapper"));
        }
        _ => panic!("expected Harness"),
    }
}

#[test]
fn runtime_isolation_rejects_allow_keychain_for_non_claude() {
    let runtimes = test_runtimes();
    let harnesses = builtin_harnesses();
    let target = match resolve_target("codex", &runtimes, &harnesses).unwrap() {
        LaunchTarget::Runtime(runtime) => runtime,
        _ => panic!("expected Runtime"),
    };

    let result = runtime_isolation_plan(target, true);

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("--allow-keychain is not supported for runtime 'Codex CLI'"));
}

#[test]
fn isolated_launch_env_uses_allowlist_and_strips_arbitrary_host_secrets() {
    let runtimes = test_runtimes();
    let harnesses = plugin_registry();
    let harness = match resolve_target("launch-plugin", &runtimes, &harnesses).unwrap() {
        LaunchTarget::Harness { harness, .. } => harness,
        _ => panic!("expected Harness"),
    };
    let paths = isolation::IsolationPaths {
        base: std::env::temp_dir().join("hm-launch-env-test/runtimes/launch-plugin"),
        home: std::env::temp_dir().join("hm-launch-env-test/runtimes/launch-plugin/home"),
        state: std::env::temp_dir().join("hm-launch-env-test/runtimes/launch-plugin/state"),
        tmp: std::env::temp_dir().join("hm-launch-env-test/runtimes/launch-plugin/tmp"),
        runtime_base: std::env::temp_dir().join("hm-launch-env-test/runtimes/codex"),
        runtime_home: std::env::temp_dir().join("hm-launch-env-test/runtimes/codex/home"),
        runtime_state: std::env::temp_dir().join("hm-launch-env-test/runtimes/codex/state"),
        runtime_logs: std::env::temp_dir().join("hm-launch-env-test/runtimes/codex/state/logs"),
    };
    let inherited = HashMap::from([
        ("PATH".to_string(), "/bin".to_string()),
        ("HOME".to_string(), "/Users/tester".to_string()),
        ("OPENAI_API_KEY".to_string(), "openai".to_string()),
        ("GITHUB_TOKEN".to_string(), "github".to_string()),
        ("NPM_TOKEN".to_string(), "npm".to_string()),
        ("SSH_AUTH_SOCK".to_string(), "/tmp/agent.sock".to_string()),
        (
            "GOOGLE_GENERATIVE_AI_API_KEY".to_string(),
            "google".to_string(),
        ),
        ("OPENROUTER_API_KEY".to_string(), "openrouter".to_string()),
    ]);

    // build_launch_env dispatches on the TARGET RUNTIME's spoof_home flag. Claude Code is the
    // only spoof_home=true runtime, so resolve it to exercise the SpoofHome sanitized path
    // (allowlist + arbitrary-secret strip). The harness isolation overlay still applies on top.
    let runtime = runtimes
        .find_by_display_name("Claude Code")
        .expect("Claude Code runtime is registered");
    let env = build_launch_env(&inherited, runtime, Some((&harness.isolation, &paths)));

    assert_eq!(env.get("PATH"), Some(&"/bin".to_string()));
    assert!(env.contains_key("HOME"));
    assert!(env.contains_key("CODEX_HOME"));
    assert!(!env.contains_key("OPENAI_API_KEY"));
    assert!(!env.contains_key("GITHUB_TOKEN"));
    assert!(!env.contains_key("NPM_TOKEN"));
    assert!(!env.contains_key("GOOGLE_GENERATIVE_AI_API_KEY"));
    assert!(!env.contains_key("OPENROUTER_API_KEY"));
    assert_eq!(
        env.get("GH_CONFIG_DIR").map(String::as_str),
        Some("/Users/tester/.config/gh")
    );
    assert_eq!(
        env.get("GIT_CONFIG_GLOBAL").map(String::as_str),
        Some("/Users/tester/.gitconfig")
    );
    assert_eq!(
        env.get("SSH_AUTH_SOCK").map(String::as_str),
        Some("/tmp/agent.sock")
    );
    assert_eq!(
        env.get("CARGO_HOME").map(String::as_str),
        Some("/Users/tester/.cargo")
    );
    assert_eq!(
        env.get("RUSTUP_HOME").map(String::as_str),
        Some("/Users/tester/.rustup")
    );
    assert_eq!(
        env.get("BUN_INSTALL").map(String::as_str),
        Some("/Users/tester/.bun")
    );
    assert_eq!(
        env.get("NPM_CONFIG_USERCONFIG").map(String::as_str),
        Some("/Users/tester/.npmrc")
    );
}

#[test]
fn resolve_target_accepts_plugin_registry_entries() {
    let runtimes = test_runtimes();
    let harnesses = plugin_registry();

    match resolve_target("launch-plugin", &runtimes, &harnesses).unwrap() {
        LaunchTarget::Harness { harness, runtime } => {
            assert_eq!(harness.display_name, "Launch Plugin");
            assert_eq!(harness.launch_binary.as_deref(), Some("plugin-wrapper"));
            assert_eq!(harness.launch_args, ["--plugin-mode"]);
            assert_eq!(runtime.name, "Codex CLI");
        }
        _ => panic!("expected Harness"),
    }
}

fn config_with_default(default: Option<&str>) -> HmConfig {
    HmConfig {
        default_profile: default.map(String::from),
        ..HmConfig::default()
    }
}

#[test]
fn effective_profile_name_uses_explicit_arg_when_provided() {
    let cfg = config_with_default(Some("default-from-config"));
    assert_eq!(
        effective_profile_name(Some("explicit"), &cfg),
        Some("explicit".to_string()),
        "explicit --profile must override default_profile"
    );
}

#[test]
fn effective_profile_name_falls_back_to_default_profile_when_arg_is_none() {
    let cfg = config_with_default(Some("proxy"));
    assert_eq!(
        effective_profile_name(None, &cfg),
        Some("proxy".to_string()),
        "default_profile must apply when no --profile passed"
    );
}

#[test]
fn effective_profile_name_is_none_when_arg_and_default_are_unset() {
    let cfg = config_with_default(None);
    assert_eq!(
        effective_profile_name(None, &cfg),
        None,
        "no profile must remain no-profile when neither arg nor default is set"
    );
}

#[test]
fn effective_profile_name_explicit_wins_when_default_unset() {
    let cfg = config_with_default(None);
    assert_eq!(
        effective_profile_name(Some("explicit"), &cfg),
        Some("explicit".to_string()),
    );
}
