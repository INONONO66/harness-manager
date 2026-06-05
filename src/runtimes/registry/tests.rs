use super::*;

#[test]
fn pi_env_var_is_not_empty() {
    let pi = RUNTIMES
        .iter()
        .find(|r| r.name == "Pi")
        .expect("Pi runtime");
    match &pi.config_locator {
        ConfigLocator::EnvOrHome { env_var, .. } => {
            assert_eq!(*env_var, "PI_CODING_AGENT_DIR");
        }
        other => panic!("Pi config_locator changed shape: {:?}", other),
    }
}

#[test]
fn phase1_runtimes_have_isolation() {
    for name in &["Codex CLI", "OpenCode", "Pi"] {
        let rt = RUNTIMES
            .iter()
            .find(|r| r.name == *name)
            .unwrap_or_else(|| panic!("{} runtime missing", name));
        assert!(
            rt.isolation.is_some(),
            "{} must have isolation in Phase 1",
            name
        );
    }
}

#[test]
fn claude_has_default_isolation_in_phase_2() {
    let c = RUNTIMES
        .iter()
        .find(|r| r.name == "Claude Code")
        .expect("Claude Code runtime");
    let iso = c.isolation.expect("Claude isolation set");
    assert_eq!(iso.subdir, "claude");
    assert!(iso
        .static_envs
        .iter()
        .any(|(k, _)| *k == "DISABLE_LOGIN_COMMAND"));
    assert!(iso
        .seed_files
        .iter()
        .any(|s| s.path.ends_with("apikey.sh") && s.overwrite && s.mode == Some(0o700)));
}

#[test]
fn claude_declares_keychain_isolation_capability() {
    let c = RUNTIMES
        .iter()
        .find(|r| r.name == "Claude Code")
        .expect("Claude Code runtime");

    assert_eq!(
        c.keychain_isolation.expect("Claude keychain isolation").subdir,
        "claude-keychain"
    );
}

#[test]
fn codex_isolation_has_seed_config() {
    let codex = RUNTIMES
        .iter()
        .find(|r| r.name == "Codex CLI")
        .expect("Codex CLI runtime");
    let iso = codex.isolation.expect("isolation set");
    assert_eq!(iso.subdir, "codex");
    assert!(iso.spoof_home);
    assert!(iso
        .seed_files
        .iter()
        .any(|s| s.path.contains("config.toml")
            && s.content.contains("analytics_enabled = false")));
}

#[test]
fn opencode_isolation_redirects_xdg_quartet_without_pure_mode() {
    let oc = RUNTIMES
        .iter()
        .find(|r| r.name == "OpenCode")
        .expect("OpenCode runtime");
    let iso = oc.isolation.expect("isolation set");
    for xdg in &[
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_CACHE_HOME",
        "XDG_STATE_HOME",
    ] {
        assert!(
            iso.static_envs.iter().any(|(k, _)| k == xdg),
            "OpenCode isolation missing {}",
            xdg
        );
    }
    assert!(
        !iso.static_envs.iter().any(|(k, _)| *k == "OPENCODE_PURE"),
        "OpenCode runtime must inherit configured plugins and MCP"
    );
}
