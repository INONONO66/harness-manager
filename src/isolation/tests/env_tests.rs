use std::collections::HashMap;

use crate::isolation::{
    build_isolation_env, build_redirect_only_env, build_sanitized_isolation_env,
};

use super::{iso_plan, tmp_paths};

#[test]
fn build_isolation_env_inserts_static_envs() {
    let p = tmp_paths("build-env");
    let spec = iso_plan(
        "test",
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

    assert!(env.get("CODEX_HOME").unwrap().ends_with("/.codex"));
    assert!(env.get("SESSION_LOG_DIR").unwrap().ends_with("/state/logs"));
    assert_eq!(env.get("PI_OFFLINE").unwrap(), "1");
}

#[test]
fn build_isolation_env_never_inserts_home() {
    let p = tmp_paths("build-env-no-home");
    let spec = iso_plan("test", &[], &[("FOO", "bar")], Vec::new(), None);

    let env = build_isolation_env(&spec, &p);

    assert!(
        !env.contains_key("HOME"),
        "build_isolation_env only overlays static_envs; HOME spoofing belongs to the sanitized builder"
    );
    assert_eq!(env.get("FOO").unwrap(), "bar");
}

#[test]
fn build_sanitized_env_strips_hostile_vars_and_uses_isolated_home() {
    let p = tmp_paths("sanitized-env");
    let spec = iso_plan(
        "test",
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
        ("HOME".to_string(), "/real/home".to_string()),
        ("PLUGIN_ROOT".to_string(), "/real/plugin".to_string()),
        ("XDG_DATA_HOME".to_string(), "/real/xdg".to_string()),
        ("XDG_CONFIG_HOME".to_string(), "/real/config".to_string()),
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
    assert_eq!(
        env.get("GH_CONFIG_DIR").map(String::as_str),
        Some("/real/config/gh")
    );
    assert_eq!(
        env.get("GIT_CONFIG_GLOBAL").map(String::as_str),
        Some("/real/home/.gitconfig")
    );
    assert_eq!(
        env.get("SSH_AUTH_SOCK").map(String::as_str),
        Some("/tmp/socket")
    );
    assert_eq!(
        env.get("CARGO_HOME").map(String::as_str),
        Some("/real/home/.cargo")
    );
    assert_eq!(
        env.get("RUSTUP_HOME").map(String::as_str),
        Some("/real/home/.rustup")
    );
    assert_eq!(
        env.get("BUN_INSTALL").map(String::as_str),
        Some("/real/home/.bun")
    );
    assert_eq!(
        env.get("NPM_CONFIG_USERCONFIG").map(String::as_str),
        Some("/real/home/.npmrc")
    );
}

#[test]
fn build_sanitized_env_preserves_explicit_host_cli_config_over_defaults() {
    let p = tmp_paths("sanitized-env-explicit-cli");
    let spec = iso_plan("test", &[], &[], Vec::new(), None);
    let inherited = HashMap::from([
        ("HOME".to_string(), "/real/home".to_string()),
        ("GH_CONFIG_DIR".to_string(), "/custom/gh".to_string()),
        (
            "GIT_CONFIG_GLOBAL".to_string(),
            "/custom/gitconfig".to_string(),
        ),
        ("CARGO_HOME".to_string(), "/custom/cargo".to_string()),
        ("RUSTUP_HOME".to_string(), "/custom/rustup".to_string()),
        ("BUN_INSTALL".to_string(), "/custom/bun".to_string()),
        (
            "NPM_CONFIG_USERCONFIG".to_string(),
            "/custom/npmrc".to_string(),
        ),
    ]);

    let env = build_sanitized_isolation_env(&inherited, &spec, &p);

    assert_eq!(
        env.get("HOME").map(String::as_str),
        Some(p.home.to_string_lossy().as_ref())
    );
    assert_eq!(
        env.get("GH_CONFIG_DIR").map(String::as_str),
        Some("/custom/gh")
    );
    assert_eq!(
        env.get("GIT_CONFIG_GLOBAL").map(String::as_str),
        Some("/custom/gitconfig")
    );
    assert_eq!(
        env.get("CARGO_HOME").map(String::as_str),
        Some("/custom/cargo")
    );
    assert_eq!(
        env.get("RUSTUP_HOME").map(String::as_str),
        Some("/custom/rustup")
    );
    assert_eq!(
        env.get("BUN_INSTALL").map(String::as_str),
        Some("/custom/bun")
    );
    assert_eq!(
        env.get("NPM_CONFIG_USERCONFIG").map(String::as_str),
        Some("/custom/npmrc")
    );
}

#[test]
fn redirect_only_keeps_home_cargo_github_token_path_shims() {
    let p = tmp_paths("redirect-only-passthrough");
    let spec = iso_plan("test", &[], &[], Vec::new(), None);
    let inherited = HashMap::from([
        ("HOME".to_string(), "/Users/test".to_string()),
        ("CARGO_HOME".to_string(), "/Users/test/.cargo".to_string()),
        ("GITHUB_TOKEN".to_string(), "ghp_secret".to_string()),
        ("ANTHROPIC_API_KEY".to_string(), "sk-ant-xxx".to_string()),
        (
            "PATH".to_string(),
            "/Users/test/.local/share/mise/shims:/usr/bin".to_string(),
        ),
    ]);

    let env = build_redirect_only_env(&inherited, &spec, &p);

    assert_eq!(env.get("HOME").map(String::as_str), Some("/Users/test"));
    assert_eq!(
        env.get("CARGO_HOME").map(String::as_str),
        Some("/Users/test/.cargo")
    );
    assert_eq!(
        env.get("GITHUB_TOKEN").map(String::as_str),
        Some("ghp_secret")
    );
    assert!(
        !env.contains_key("ANTHROPIC_API_KEY"),
        "AI credential must be stripped in RedirectOnly"
    );
    assert!(
        env.get("PATH").unwrap().contains("mise/shims"),
        "RedirectOnly must preserve mise shims in PATH"
    );
}

#[test]
fn sanitized_isolation_replaces_home_and_strips_github_token() {
    let p = tmp_paths("sanitized-contrast");
    let spec = iso_plan("test", &[], &[], Vec::new(), None);
    let inherited = HashMap::from([
        ("HOME".to_string(), "/Users/test".to_string()),
        ("CARGO_HOME".to_string(), "/Users/test/.cargo".to_string()),
        ("GITHUB_TOKEN".to_string(), "ghp_secret".to_string()),
        ("ANTHROPIC_API_KEY".to_string(), "sk-ant-xxx".to_string()),
        (
            "PATH".to_string(),
            "/Users/test/.local/share/mise/shims:/usr/bin".to_string(),
        ),
    ]);

    let env = build_sanitized_isolation_env(&inherited, &spec, &p);

    assert_eq!(
        env.get("HOME").map(String::as_str),
        Some(p.home.to_string_lossy().as_ref())
    );
    assert_ne!(env.get("HOME").map(String::as_str), Some("/Users/test"));
    assert!(
        !env.contains_key("GITHUB_TOKEN"),
        "SpoofHome must strip host secret GITHUB_TOKEN"
    );
    assert!(!env.contains_key("ANTHROPIC_API_KEY"));
    assert!(
        !env.get("PATH").unwrap().contains("mise/shims"),
        "SpoofHome must strip mise shims from PATH"
    );
    // CARGO_HOME is preserved (not stripped) via the SAFE_INHERITED_ENV allowlist; asserting
    // it survives guards against a regression that drops host toolchain access.
    assert_eq!(
        env.get("CARGO_HOME").map(String::as_str),
        Some("/Users/test/.cargo")
    );
}

#[test]
fn redirect_only_credential_strip_has_no_config_path_vars() {
    let config_path_vars = [
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_CACHE_HOME",
        "XDG_STATE_HOME",
        "CODEX_HOME",
        "CLAUDE_CONFIG_DIR",
        "OPENCODE_CONFIG_DIR",
        "PI_CODING_AGENT_DIR",
    ];
    for var in config_path_vars {
        assert!(
            !crate::isolation::env::REDIRECT_ONLY_CREDENTIAL_STRIP.contains(&var),
            "config-path var {} found in REDIRECT_ONLY_CREDENTIAL_STRIP",
            var
        );
    }
}

#[test]
fn redirect_only_credential_strip_contains_ai_credentials() {
    let credential_vars = [
        "ANTHROPIC_API_KEY",
        "ANTHROPIC_AUTH_TOKEN",
        "OPENAI_API_KEY",
        "CODEX_API_KEY",
        "GOOGLE_API_KEY",
        "GROQ_API_KEY",
    ];
    for var in credential_vars {
        assert!(
            crate::isolation::env::REDIRECT_ONLY_CREDENTIAL_STRIP.contains(&var),
            "credential var {} missing from REDIRECT_ONLY_CREDENTIAL_STRIP",
            var
        );
    }
}

#[test]
fn redirect_only_overlays_isolation_static_envs() {
    let p = tmp_paths("redirect-only-overlay");
    // RedirectOnly: isolation static_envs still overlay (with token substitution).
    let spec = iso_plan(
        "test",
        &[],
        &[("CODEX_HOME", "{home}/.codex")],
        Vec::new(),
        None,
    );
    let inherited = HashMap::from([
        ("HOME".to_string(), "/Users/test".to_string()),
        ("PATH".to_string(), "/usr/bin".to_string()),
    ]);

    let env = build_redirect_only_env(&inherited, &spec, &p);

    // static_envs overlaid with {home} substituted to the isolation home.
    assert_eq!(
        env.get("CODEX_HOME").map(String::as_str),
        Some(p.home.join(".codex").to_string_lossy().as_ref())
    );
    // RedirectOnly: HOME stays the host value (not overridden by the overlay).
    assert_eq!(env.get("HOME").map(String::as_str), Some("/Users/test"));
    // No PATH filtering in RedirectOnly.
    assert_eq!(env.get("PATH").map(String::as_str), Some("/usr/bin"));
}
