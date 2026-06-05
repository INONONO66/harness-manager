use super::*;
use std::collections::HashMap;

fn plugin_registry() -> crate::harnesses::registry::HarnessRegistry {
    crate::harnesses::registry::HarnessRegistry::from_sources(&[
        crate::harnesses::registry::HarnessSource::manifest(
            "launch-plugin.toml",
            r#"
schema_version = 1
id = "launch-plugin"
display_name = "Launch Plugin"
target_runtime = "Codex CLI"
detect_binaries = ["launch-plugin-bin"]
launch_binary = "plugin-wrapper"
launch_args = ["--plugin-mode"]

[package]
kind = "manual"
instructions = "manual"

[isolation]
spoof_home = true
home_subdirs = [".codex"]
static_envs = { CODEX_HOME = "{home}/.codex" }
seed_files = []
"#,
        ),
    ])
    .unwrap()
}

#[test]
fn resolve_target_runtime() {
    let registry = crate::harnesses::registry::HarnessRegistry::builtin_only().unwrap();
    match resolve_target("codex", &registry).unwrap() {
        LaunchTarget::Runtime(rt) => assert_eq!(rt.name, "Codex CLI"),
        _ => panic!("expected Runtime"),
    }
}

#[test]
fn resolve_target_runtime_by_name() {
    let registry = crate::harnesses::registry::HarnessRegistry::builtin_only().unwrap();
    match resolve_target("Codex CLI", &registry).unwrap() {
        LaunchTarget::Runtime(rt) => assert_eq!(rt.name, "Codex CLI"),
        _ => panic!("expected Runtime"),
    }
}

#[test]
fn resolve_target_harness() {
    let registry = plugin_registry();
    match resolve_target("launch-plugin", &registry).unwrap() {
        LaunchTarget::Harness { harness, runtime } => {
            assert_eq!(harness.id, "launch-plugin");
            assert_eq!(runtime.name, "Codex CLI");
        }
        _ => panic!("expected Harness"),
    }
}

#[test]
fn resolve_target_harness_case_insensitive() {
    let registry = plugin_registry();
    match resolve_target("LAUNCH-PLUGIN", &registry).unwrap() {
        LaunchTarget::Harness { harness, .. } => assert_eq!(harness.id, "launch-plugin"),
        _ => panic!("expected Harness"),
    }
}

#[test]
fn resolve_target_unknown() {
    let registry = crate::harnesses::registry::HarnessRegistry::builtin_only().unwrap();
    assert!(resolve_target("nonexistent-xyz", &registry).is_err());
}

#[test]
fn resolve_target_plugin_harness_has_wrapper() {
    let registry = plugin_registry();
    match resolve_target("launch-plugin", &registry).unwrap() {
        LaunchTarget::Harness { harness, .. } => {
            assert_eq!(harness.launch_binary.as_deref(), Some("plugin-wrapper"));
        }
        _ => panic!("expected Harness"),
    }
}

#[test]
fn runtime_isolation_rejects_allow_keychain_for_non_claude() {
    let registry = crate::harnesses::registry::HarnessRegistry::builtin_only().unwrap();
    let target = match resolve_target("codex", &registry).unwrap() {
        LaunchTarget::Runtime(runtime) => runtime,
        _ => panic!("expected Runtime"),
    };

    let result = runtime_isolation(target, true);

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("--allow-keychain is not supported for runtime 'Codex CLI'"));
}

#[test]
fn isolated_launch_env_uses_allowlist_and_strips_arbitrary_host_secrets() {
    let registry = plugin_registry();
    let target = match resolve_target("launch-plugin", &registry).unwrap() {
        LaunchTarget::Harness { harness, runtime } => (harness, runtime),
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

    let env = build_launch_env(&inherited, target.1, Some((&target.0.isolation, &paths)));

    assert_eq!(env.get("PATH"), Some(&"/bin".to_string()));
    assert!(env.contains_key("HOME"));
    assert!(env.contains_key("CODEX_HOME"));
    assert!(!env.contains_key("OPENAI_API_KEY"));
    assert!(!env.contains_key("GITHUB_TOKEN"));
    assert!(!env.contains_key("NPM_TOKEN"));
    assert!(!env.contains_key("SSH_AUTH_SOCK"));
    assert!(!env.contains_key("GOOGLE_GENERATIVE_AI_API_KEY"));
    assert!(!env.contains_key("OPENROUTER_API_KEY"));
}

#[test]
fn resolve_target_accepts_plugin_registry_entries() {
    let registry = plugin_registry();

    match resolve_target("launch-plugin", &registry).unwrap() {
        LaunchTarget::Harness { harness, runtime } => {
            assert_eq!(harness.display_name, "Launch Plugin");
            assert_eq!(harness.launch_binary.as_deref(), Some("plugin-wrapper"));
            assert_eq!(harness.launch_args, ["--plugin-mode"]);
            assert_eq!(runtime.name, "Codex CLI");
        }
        _ => panic!("expected Harness"),
    }
}
