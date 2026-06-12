use std::fs;

use super::dynamic::{RuntimeDiscoveryEnv, RuntimeRegistry, RuntimeSource};

fn minimal_runtime_manifest(name: &str, binary: &str) -> String {
    format!(
        r#"
schema_version = 1
name = "{name}"
binary_names = ["{binary}"]
version_arg = "--version"

[config_locator]
kind = "env-or-home"
env = ""
home_relative = ".{binary}"

[auth_login]
kind = "unsupported"

[isolation]
subdir = "{binary}"
spoof_home = true
"#
    )
}

#[test]
fn builtin_only_returns_expected_runtimes() {
    let registry = RuntimeRegistry::builtin_only().expect("builtin runtimes parse");
    let names: Vec<&str> = registry.records().iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"Claude Code"));
    assert!(names.contains(&"Codex CLI"));
    assert!(names.contains(&"Gajae-Code"));
    assert!(names.contains(&"Grok CLI"));
    assert!(names.contains(&"OpenCode"));
    assert!(names.contains(&"Pi"));
    assert_eq!(registry.records().len(), 6);
}

#[test]
fn find_by_binary_and_display_name() {
    let registry = RuntimeRegistry::builtin_only().unwrap();
    assert_eq!(registry.find("claude").unwrap().name, "Claude Code");
    assert_eq!(registry.find("Claude Code").unwrap().name, "Claude Code");
    assert_eq!(registry.find("codex").unwrap().name, "Codex CLI");
    assert_eq!(registry.find("Codex CLI").unwrap().name, "Codex CLI");
    assert_eq!(registry.find("CODEX-CLI").unwrap().name, "Codex CLI");
    assert_eq!(registry.find("gjc").unwrap().name, "Gajae-Code");
    assert_eq!(registry.find("grok").unwrap().name, "Grok CLI");
    assert!(registry.find("nope").is_none());
}

#[test]
fn find_by_display_name_is_exact_match() {
    let registry = RuntimeRegistry::builtin_only().unwrap();
    assert!(registry.find_by_display_name("Claude Code").is_some());
    assert!(registry.find_by_display_name("claude").is_none());
}

#[test]
fn id_conflicts_with_runtime_blocks_runtime_binary_and_display_name() {
    let registry = RuntimeRegistry::builtin_only().unwrap();
    assert!(registry.id_conflicts_with_runtime("codex"));
    assert!(registry.id_conflicts_with_runtime("claudecode"));
    assert!(!registry.id_conflicts_with_runtime("my-harness"));
}

#[test]
fn user_manifest_overrides_builtin_display_name() {
    let registry = RuntimeRegistry::from_sources(&[
        RuntimeSource::builtins(),
        RuntimeSource::manifest(
            "dup.toml",
            minimal_runtime_manifest("Claude Code", "claude2"),
        ),
    ])
    .unwrap();
    let claude = registry.find_by_display_name("Claude Code").unwrap();
    assert_eq!(
        claude.binary_names,
        vec!["claude2".to_string()],
        "user manifest must replace builtin claude"
    );
    let preserved = registry
        .find("claude")
        .expect("old 'claude' binary route preserved as detection alias on replacement");
    assert_eq!(preserved.name, "Claude Code");
    assert_eq!(preserved.binary_names, vec!["claude2".to_string()]);
    assert_eq!(registry.records().len(), 6, "claude replaced, not added");
}

#[test]
fn user_manifest_overrides_builtin_binary_name() {
    let registry = RuntimeRegistry::from_sources(&[
        RuntimeSource::builtins(),
        RuntimeSource::manifest("dup.toml", minimal_runtime_manifest("Claude Two", "claude")),
    ])
    .unwrap();
    let claude_two = registry.find_by_display_name("Claude Two").unwrap();
    assert_eq!(claude_two.binary_names, vec!["claude".to_string()]);
    assert!(
        registry.find_by_display_name("Claude Code").is_none(),
        "original Claude Code gone after binary override"
    );
}

#[test]
fn user_manifest_overrides_normalized_shadow_of_builtin_display_name() {
    let registry = RuntimeRegistry::from_sources(&[
        RuntimeSource::builtins(),
        RuntimeSource::manifest(
            "shadow.toml",
            minimal_runtime_manifest("CLAUDE CODE", "claude-shadow"),
        ),
    ])
    .unwrap();
    assert!(registry.find_by_display_name("CLAUDE CODE").is_some());
    assert!(registry.find_by_display_name("Claude Code").is_none());
}

#[test]
fn user_manifest_overrides_normalized_shadow_of_builtin_binary() {
    let registry = RuntimeRegistry::from_sources(&[
        RuntimeSource::builtins(),
        RuntimeSource::manifest(
            "shadow.toml",
            minimal_runtime_manifest("Imposter Codex", "Codex"),
        ),
    ])
    .unwrap();
    let imposter = registry.find_by_display_name("Imposter Codex").unwrap();
    assert_eq!(imposter.binary_names, vec!["Codex".to_string()]);
    assert!(registry.find_by_display_name("Codex CLI").is_none());
}

#[test]
fn rejects_duplicate_user_manifests_on_same_normalized_route() {
    let err = RuntimeRegistry::from_sources(&[
        RuntimeSource::manifest("first.toml", minimal_runtime_manifest("Alpha", "alpha-bin")),
        RuntimeSource::manifest("second.toml", minimal_runtime_manifest("alpha", "beta-bin")),
    ])
    .unwrap_err();
    assert!(
        err.to_string().contains("duplicate runtime route"),
        "expected duplicate-route error between two user manifests, got: {err:#}"
    );
}

#[test]
fn rejects_user_manifest_shadowing_multiple_builtins() {
    // display "Claude Code" shadows builtin Claude; binary "codex" shadows builtin Codex.
    let err = RuntimeRegistry::from_sources(&[
        RuntimeSource::builtins(),
        RuntimeSource::manifest(
            "evil.toml",
            minimal_runtime_manifest("Claude Code", "codex"),
        ),
    ])
    .unwrap_err();

    let msg = err.to_string();
    assert!(
        msg.contains("shadow multiple"),
        "expected multi-shadow error, got: {msg}"
    );
    assert!(
        msg.contains("Claude Code"),
        "error should name Claude Code: {msg}"
    );
    assert!(
        msg.contains("Codex CLI"),
        "error should name Codex CLI: {msg}"
    );
    assert!(
        msg.contains("evil.toml"),
        "error should name the offending user manifest label: {msg}"
    );
}

#[test]
fn identical_user_copy_of_builtin_loads_without_changing_count() {
    use crate::runtimes::builtin::BUILTIN_RUNTIME_MANIFESTS;

    let (_, builtin_content) = BUILTIN_RUNTIME_MANIFESTS[0];

    let registry = RuntimeRegistry::from_sources(&[
        RuntimeSource::builtins(),
        RuntimeSource::manifest("identical.toml", builtin_content.to_string()),
    ])
    .unwrap();

    assert_eq!(
        registry.records().len(),
        6,
        "identical override replaces builtin, count unchanged",
    );
}

#[test]
fn find_resolves_old_display_name_after_binary_route_override() {
    let registry = RuntimeRegistry::from_sources(&[
        RuntimeSource::builtins(),
        RuntimeSource::manifest("aaa.toml", minimal_runtime_manifest("Aaa Claude", "claude")),
    ])
    .unwrap();

    let by_old_display = registry
        .find("Claude Code")
        .expect("old display 'Claude Code' must still resolve via preserved route");
    assert_eq!(by_old_display.name, "Aaa Claude");

    let by_normalized = registry
        .find("claudecode")
        .expect("normalized old display must still resolve");
    assert_eq!(by_normalized.name, "Aaa Claude");

    assert!(
        registry.find_by_display_name("Claude Code").is_none(),
        "find_by_display_name must stay strict, not see the preserved route alias",
    );
    assert!(
        registry.find_by_display_name("Aaa Claude").is_some(),
        "new user display name must be the only direct display match",
    );
}

#[test]
fn discovery_picks_up_user_runtime_under_xdg_config_home() {
    let temp = tempfile::tempdir().unwrap();
    let runtime_dir = temp.path().join("config").join("hm").join("runtimes.d");
    fs::create_dir_all(&runtime_dir).unwrap();
    fs::write(
        runtime_dir.join("example.toml"),
        minimal_runtime_manifest("Example Runtime", "example-bin"),
    )
    .unwrap();

    let registry = RuntimeRegistry::load_from_env(&RuntimeDiscoveryEnv {
        xdg_config_home: Some(temp.path().join("config")),
        xdg_data_home: Some(temp.path().join("data")),
        home: Some(temp.path().join("home")),
    })
    .unwrap();

    assert!(registry.find("example-bin").is_some());
    assert!(registry.find_by_display_name("Example Runtime").is_some());
    assert!(registry.records().len() > 6);
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_user_manifest() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let config_root = temp.path().join("config");
    let runtime_dir = config_root.join("hm").join("runtimes.d");
    let outside = temp.path().join("outside.toml");
    fs::create_dir_all(&runtime_dir).unwrap();
    fs::write(
        &outside,
        minimal_runtime_manifest("Outside Runtime", "outside-bin"),
    )
    .unwrap();
    symlink(&outside, runtime_dir.join("escape.toml")).unwrap();

    let err = RuntimeRegistry::load_from_env(&RuntimeDiscoveryEnv {
        xdg_config_home: Some(config_root),
        xdg_data_home: Some(temp.path().join("data")),
        home: Some(temp.path().join("home")),
    })
    .unwrap_err();

    assert!(
        err.to_string().to_lowercase().contains("symlink"),
        "expected symlink rejection, got: {err:#}"
    );
}

#[cfg(unix)]
#[test]
fn rejects_oversized_runtime_manifest_before_reading_contents() {
    use std::os::unix::fs::PermissionsExt;

    // Given: an oversized runtime manifest that cannot be read by this process.
    let temp = tempfile::tempdir().unwrap();
    let config_root = temp.path().join("config");
    let runtime_dir = config_root.join("hm").join("runtimes.d");
    let oversized = runtime_dir.join("oversized.toml");
    fs::create_dir_all(&runtime_dir).unwrap();
    fs::write(&oversized, "x".repeat(70 * 1024)).unwrap();
    let mut permissions = fs::metadata(&oversized).unwrap().permissions();
    permissions.set_mode(0o000);
    fs::set_permissions(&oversized, permissions).unwrap();

    // When: the registry discovers that manifest.
    let err = RuntimeRegistry::load_from_env(&RuntimeDiscoveryEnv {
        xdg_config_home: Some(config_root),
        xdg_data_home: Some(temp.path().join("data")),
        home: Some(temp.path().join("home")),
    })
    .unwrap_err();

    let mut permissions = fs::metadata(&oversized).unwrap().permissions();
    permissions.set_mode(0o600);
    fs::set_permissions(&oversized, permissions).unwrap();

    // Then: it rejects by metadata size before attempting to read contents.
    assert!(
        err.to_string().contains("exceeds 64 KiB"),
        "expected pre-read size rejection, got: {err:#}"
    );
}
