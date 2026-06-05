use std::collections::HashMap;

use crate::isolation::{build_isolation_env, build_sanitized_isolation_env};

use super::{iso_plan, tmp_paths};

#[test]
fn build_env_inserts_home_and_static_envs() {
    let p = tmp_paths("build-env");
    let spec = iso_plan(
        "test",
        true,
        &[],
        &[
            ("CODEX_HOME", "{home}/.codex"),
            ("SESSION_LOG_DIR", "{runtime_logs}"),
            ("PI_OFFLINE", "1"),
        ],
        Vec::new(),
        None,
    );

    let env = build_isolation_env(&spec, &p);

    assert_eq!(
        env.get("HOME").unwrap(),
        &p.home.to_string_lossy().to_string()
    );
    assert!(env.get("CODEX_HOME").unwrap().ends_with("/.codex"));
    assert!(env.get("SESSION_LOG_DIR").unwrap().ends_with("/state/logs"));
    assert_eq!(env.get("PI_OFFLINE").unwrap(), "1");
}

#[test]
fn build_env_skips_home_when_spoof_disabled() {
    let p = tmp_paths("build-env-no-spoof");
    let spec = iso_plan("test", false, &[], &[("FOO", "bar")], Vec::new(), None);

    let env = build_isolation_env(&spec, &p);

    assert!(!env.contains_key("HOME"));
    assert_eq!(env.get("FOO").unwrap(), "bar");
}

#[test]
fn build_sanitized_env_strips_hostile_vars_and_uses_isolated_home() {
    let p = tmp_paths("sanitized-env");
    let spec = iso_plan(
        "test",
        true,
        &[],
        &[("CODEX_HOME", "{home}/.codex")],
        Vec::new(),
        None,
    );
    let inherited = HashMap::from([
        ("PATH".to_string(), "/bin".to_string()),
        ("OPENAI_API_KEY".to_string(), "leak".to_string()),
        ("CODEX_ACCESS_TOKEN".to_string(), "leak".to_string()),
        ("ANTHROPIC_API_KEY".to_string(), "leak".to_string()),
        ("PLUGIN_ROOT".to_string(), "/real/plugin".to_string()),
        ("XDG_DATA_HOME".to_string(), "/real/xdg".to_string()),
        ("NPM_TOKEN".to_string(), "leak".to_string()),
        ("SSH_AUTH_SOCK".to_string(), "/tmp/socket".to_string()),
    ]);

    let env = build_sanitized_isolation_env(&inherited, &spec, &p);

    assert_eq!(env.get("PATH").map(String::as_str), Some("/bin"));
    assert_eq!(
        env.get("HOME").map(String::as_str),
        Some(p.home.to_string_lossy().as_ref())
    );
    assert_eq!(
        env.get("CODEX_HOME").map(String::as_str),
        Some(p.home.join(".codex").to_string_lossy().as_ref())
    );
    assert!(!env.contains_key("OPENAI_API_KEY"));
    assert!(!env.contains_key("CODEX_ACCESS_TOKEN"));
    assert!(!env.contains_key("ANTHROPIC_API_KEY"));
    assert!(!env.contains_key("PLUGIN_ROOT"));
    assert!(!env.contains_key("XDG_DATA_HOME"));
    assert!(!env.contains_key("NPM_TOKEN"));
    assert!(!env.contains_key("SSH_AUTH_SOCK"));
}
