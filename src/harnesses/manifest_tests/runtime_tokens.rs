use std::collections::BTreeMap;

use super::{minimal_manifest, parse_toml};
use crate::harnesses::builtin::builtin_specs;

#[test]
fn manifest_allows_runtime_log_token_for_static_envs() {
    // Given: a harness env var that should point at the target runtime's shared log root.
    let input = minimal_manifest("").replace(
        r#"static_envs = { CODEX_HOME = "{home}/.codex" }"#,
        r#"static_envs = { CODEX_HOME = "{home}/.codex", DEMO_LOGS = "{runtime_logs}" }"#,
    );

    // When: the manifest is parsed.
    let parsed = parse_toml("runtime-logs.toml", &input).expect("runtime log token parses");

    // Then: the harness keeps its own isolation root while recording the target runtime root.
    assert_eq!(parsed.isolation.subdir, "demo");
    assert_eq!(parsed.isolation.runtime_subdir, "codex");
    assert!(parsed
        .isolation
        .static_envs
        .iter()
        .any(|(key, value)| key == "DEMO_LOGS" && value == "{runtime_logs}"));
}

#[test]
fn manifest_allows_runtime_home_token_for_static_envs_and_seed_files() {
    // Given: a harness that writes runtime-owned config into the target runtime home.
    let input = minimal_manifest("").replace(
        r#"static_envs = { CODEX_HOME = "{home}/.codex" }"#,
        r#"static_envs = { CODEX_HOME = "{runtime_home}/.codex" }

[[isolation.seed_files]]
path = "{runtime_home}/.codex/config.toml"
content = "analytics_enabled = false\n"
overwrite = false"#,
    );

    // When: the manifest is parsed.
    let parsed = parse_toml("runtime-home.toml", &input).expect("runtime home token parses");

    // Then: runtime-owned env and seed paths are preserved for later token substitution.
    assert_eq!(parsed.isolation.runtime_subdir, "codex");
    assert_eq!(
        parsed.isolation.static_envs,
        vec![(
            "CODEX_HOME".to_string(),
            "{runtime_home}/.codex".to_string()
        )]
    );
    assert_eq!(parsed.isolation.seed_files[0].path, "{runtime_home}/.codex/config.toml");
}

#[test]
fn bundled_codex_harnesses_share_runtime_home() {
    // Given: bundled manifests that target Codex.
    let specs = builtin_specs().expect("builtins parse");
    let codex_specs: Vec<_> = specs
        .iter()
        .filter(|spec| spec.target_runtime == "Codex CLI")
        .collect();

    // When/Then: every Codex harness shares the runtime home and keeps overlay .codex absent.
    assert!(!codex_specs.is_empty(), "expected bundled Codex harnesses");
    for spec in codex_specs {
        assert_eq!(spec.isolation.runtime_subdir, "codex");
        assert!(
            !spec.isolation.home_subdirs.iter().any(|subdir| subdir == ".codex"),
            "{} must not pre-create a harness-local .codex directory",
            spec.id
        );
        assert!(
            spec.isolation
                .static_envs
                .iter()
                .any(|(key, value)| key == "CODEX_HOME" && value == "{runtime_home}/.codex"),
            "{} must point CODEX_HOME at the shared Codex runtime home",
            spec.id
        );
    }
}

#[test]
fn bundled_opencode_harnesses_share_runtime_home() {
    // Given: bundled manifests that target OpenCode.
    let specs = builtin_specs().expect("builtins parse");
    let opencode_specs: Vec<_> = specs
        .iter()
        .filter(|spec| spec.target_runtime == "OpenCode")
        .collect();

    // When/Then: every OpenCode harness uses runtime XDG roots and avoids overlay OpenCode state.
    assert!(!opencode_specs.is_empty(), "expected bundled OpenCode harnesses");
    for spec in opencode_specs {
        let envs: BTreeMap<_, _> = spec.isolation.static_envs.iter().cloned().collect();
        assert_eq!(spec.detect_binaries, vec!["opencode"]);
        assert_eq!(spec.launch_binary.as_deref(), Some("opencode"));
        assert!(
            !spec
                .isolation
                .home_subdirs
                .iter()
                .any(|subdir| subdir.contains("opencode")),
            "{} must not pre-create harness-local OpenCode state directories",
            spec.id
        );
        assert_eq!(envs.get("XDG_CONFIG_HOME").map(String::as_str), Some("{runtime_home}/.config"));
        assert_eq!(
            envs.get("XDG_DATA_HOME").map(String::as_str),
            Some("{runtime_home}/.local/share")
        );
        assert_eq!(envs.get("XDG_CACHE_HOME").map(String::as_str), Some("{runtime_home}/.cache"));
        assert_eq!(
            envs.get("XDG_STATE_HOME").map(String::as_str),
            Some("{runtime_home}/.local/state")
        );
        assert_eq!(
            envs.get("OPENCODE_CONFIG_DIR").map(String::as_str),
            Some("{runtime_home}/.config/opencode")
        );
        assert!(!envs.contains_key("OPENCODE_PURE"));
    }
}

#[test]
fn bundled_claude_harnesses_share_runtime_home() {
    // Given: bundled manifests that target Claude.
    let specs = builtin_specs().expect("builtins parse");
    let claude_specs: Vec<_> = specs
        .iter()
        .filter(|spec| spec.target_runtime == "Claude Code")
        .collect();

    // When/Then: every Claude harness inherits runtime config/plugin state.
    assert!(!claude_specs.is_empty(), "expected bundled Claude harnesses");
    for spec in claude_specs {
        let envs: BTreeMap<_, _> = spec.isolation.static_envs.iter().cloned().collect();
        assert_eq!(spec.isolation.runtime_subdir, "claude");
        assert!(
            !spec
                .isolation
                .home_subdirs
                .iter()
                .any(|subdir| subdir.starts_with(".claude")),
            "{} must not pre-create harness-local Claude state directories",
            spec.id
        );
        assert_eq!(
            envs.get("CLAUDE_CONFIG_DIR").map(String::as_str),
            Some("{runtime_home}/.claude")
        );
        assert!(
            spec.isolation
                .seed_files
                .iter()
                .any(|seed| seed.path == "{runtime_home}/.claude/settings.json"),
            "{} must seed shared Claude runtime settings",
            spec.id
        );
    }
}

