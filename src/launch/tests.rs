use super::*;
use std::collections::HashMap;

#[test]
fn resolve_target_runtime() {
    match resolve_target("codex").unwrap() {
        LaunchTarget::Runtime(rt) => assert_eq!(rt.name, "Codex CLI"),
        _ => panic!("expected Runtime"),
    }
}

#[test]
fn resolve_target_runtime_by_name() {
    match resolve_target("Codex CLI").unwrap() {
        LaunchTarget::Runtime(rt) => assert_eq!(rt.name, "Codex CLI"),
        _ => panic!("expected Runtime"),
    }
}

#[test]
fn resolve_target_harness() {
    match resolve_target("omx").unwrap() {
        LaunchTarget::Harness { harness, runtime } => {
            assert_eq!(harness.id, "omx");
            assert_eq!(runtime.name, "Codex CLI");
        }
        _ => panic!("expected Harness"),
    }
}

#[test]
fn resolve_target_harness_case_insensitive() {
    match resolve_target("OMX").unwrap() {
        LaunchTarget::Harness { harness, .. } => assert_eq!(harness.id, "omx"),
        _ => panic!("expected Harness"),
    }
}

#[test]
fn resolve_target_unknown() {
    assert!(resolve_target("nonexistent-xyz").is_err());
}

#[test]
fn resolve_target_omc_targets_claude() {
    match resolve_target("omc").unwrap() {
        LaunchTarget::Harness { harness, runtime } => {
            assert_eq!(harness.id, "omc");
            assert_eq!(runtime.name, "Claude Code");
        }
        _ => panic!("expected Harness"),
    }
}

#[test]
fn resolve_target_lazycodex_has_wrapper() {
    match resolve_target("lazycodex").unwrap() {
        LaunchTarget::Harness { harness, .. } => {
            assert_eq!(harness.launch_binary, Some("lazycodex-ai"));
        }
        _ => panic!("expected Harness"),
    }
}

#[test]
fn runtime_isolation_rejects_allow_keychain_for_non_claude() {
    let target = match resolve_target("codex").unwrap() {
        LaunchTarget::Runtime(runtime) => runtime,
        _ => panic!("expected Runtime"),
    };

    let result = runtime_isolation(target, true);

    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("--allow-keychain is only supported for Claude Code"));
}

#[test]
fn isolated_launch_env_uses_allowlist_and_strips_arbitrary_host_secrets() {
    let target = match resolve_target("omx").unwrap() {
        LaunchTarget::Harness { harness, runtime } => (harness, runtime),
        _ => panic!("expected Harness"),
    };
    let paths = isolation::IsolationPaths {
        base: std::env::temp_dir().join("hm-launch-env-test/runtimes/omx"),
        home: std::env::temp_dir().join("hm-launch-env-test/runtimes/omx/home"),
        state: std::env::temp_dir().join("hm-launch-env-test/runtimes/omx/state"),
        tmp: std::env::temp_dir().join("hm-launch-env-test/runtimes/omx/tmp"),
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
