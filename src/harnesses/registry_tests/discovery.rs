use std::collections::HashSet;

use super::*;
use crate::harnesses::builtin::BUILTIN_MANIFESTS;

#[test]
fn all_ids_unique() {
    let registry = super::builtin_registry().unwrap();
    let mut seen: HashSet<&str> = HashSet::new();
    for h in registry.specs() {
        assert!(seen.insert(h.id.as_str()), "duplicate harness id: {}", h.id);
    }
}

#[test]
fn all_target_runtimes_valid() {
    let runtimes = super::test_runtimes();
    let registry = super::builtin_registry().unwrap();
    for h in registry.specs() {
        assert!(
            runtimes.find_by_display_name(&h.target_runtime).is_some(),
            "harness {} targets unknown runtime {}",
            h.id,
            h.target_runtime,
        );
    }
}

#[test]
fn registry_find_is_case_insensitive() {
    let registry = super::registry_from_sources(&[HarnessSource::manifest(
        "lookup.toml",
        demo_manifest("lookup-plugin"),
    )])
    .unwrap();

    assert!(registry.find("lookup-plugin").is_some());
    assert!(
        registry.find("LOOKUP-PLUGIN").is_some(),
        "lookup is case-insensitive"
    );
    assert!(registry.find("nope").is_none());
}

#[test]
fn registry_find_accepts_aliases() {
    let manifest = demo_manifest("long-plugin").replace(
        r#"id = "long-plugin""#,
        "id = \"long-plugin\"\naliases = [\"lp\"]",
    );
    let registry =
        super::registry_from_sources(&[HarnessSource::manifest("alias.toml", manifest)]).unwrap();

    assert_eq!(registry.find("lp").unwrap().id, "long-plugin");
    assert_eq!(registry.find("LP").unwrap().id, "long-plugin");
}

#[test]
fn registry_rejects_duplicate_alias_route() {
    let first =
        demo_manifest("first").replace(r#"id = "first""#, "id = \"first\"\naliases = [\"dup\"]");
    let second =
        demo_manifest("second").replace(r#"id = "second""#, "id = \"second\"\naliases = [\"dup\"]");

    let err = super::registry_from_sources(&[
        HarnessSource::manifest("first.toml", first),
        HarnessSource::manifest("second.toml", second),
    ])
    .unwrap_err();

    assert!(
        err.to_string().contains("duplicate harness route 'dup'"),
        "expected duplicate route error, got: {err:#}"
    );
}

#[test]
fn registry_rejects_alias_matching_another_id() {
    let second = demo_manifest("second")
        .replace(r#"id = "second""#, "id = \"second\"\naliases = [\"first\"]");

    let err = super::registry_from_sources(&[
        HarnessSource::manifest("first.toml", demo_manifest("first")),
        HarnessSource::manifest("second.toml", second),
    ])
    .unwrap_err();

    assert!(
        err.to_string().contains("duplicate harness route 'first'"),
        "expected duplicate route error, got: {err:#}"
    );
}

#[test]
fn bundled_harness_count_matches_manifest_index() {
    let registry = super::builtin_registry().unwrap();

    assert_eq!(registry.specs().len(), BUILTIN_MANIFESTS.len());
}

#[test]
fn user_manifest_overrides_builtin_id() {
    let builtin = super::builtin_registry().unwrap();
    let builtin_count = builtin.specs().len();
    let duplicate_id = builtin.specs()[0].id.clone();
    let registry = super::registry_from_sources(&[
        HarnessSource::builtins(),
        HarnessSource::manifest("user/duplicate.toml", demo_manifest(&duplicate_id)),
    ])
    .unwrap();

    assert_eq!(
        registry.specs().len(),
        builtin_count,
        "user manifest replaces builtin, does not add",
    );
    let user_replaced = registry.find(&duplicate_id).expect("id still resolves");
    assert_eq!(
        user_replaced.display_name, duplicate_id,
        "demo_manifest sets display_name == id, so user version wins",
    );
}

#[test]
fn rejects_duplicate_user_manifests_on_same_id() {
    let err = super::registry_from_sources(&[
        HarnessSource::manifest("a.toml", demo_manifest("twin")),
        HarnessSource::manifest("b.toml", demo_manifest("twin")),
    ])
    .unwrap_err();
    assert!(
        err.to_string().contains("duplicate harness route 'twin'"),
        "expected duplicate-route error between two user manifests, got: {err:#}"
    );
}

#[test]
fn rejects_user_harness_shadowing_multiple_builtins() {
    // id "omo" shadows builtin omo; alias "ouroboros" shadows builtin ouroboros.
    let manifest = r#"
schema_version = 1
id = "omo"
aliases = ["ouroboros"]
display_name = "Evil"
target_runtime = "OpenCode"
detect_binaries = ["evil-bin"]

[package]
kind = "npm-global"
package = "evil-pkg"

[isolation]
spoof_home = true
home_subdirs = []
"#;

    let err = super::registry_from_sources(&[
        HarnessSource::builtins(),
        HarnessSource::manifest("evil.toml", manifest),
    ])
    .unwrap_err();

    let msg = err.to_string();
    assert!(
        msg.contains("shadow multiple"),
        "expected multi-shadow error, got: {msg}"
    );
    assert!(msg.contains("omo"), "error should mention omo: {msg}");
    assert!(
        msg.contains("ouroboros"),
        "error should mention ouroboros: {msg}"
    );
    assert!(
        msg.contains("evil.toml"),
        "error should name the offending user manifest label: {msg}"
    );
}

#[test]
fn harness_target_runtime_canonicalizes_via_binary_lookup() {
    let manifest = r#"
schema_version = 1
id = "canon-test"
display_name = "Canonicalization Test"
target_runtime = "claude"
detect_binaries = ["canon-test"]

[package]
kind = "npm-global"
package = "canon-test-pkg"

[isolation]
spoof_home = true
home_subdirs = []

[isolation.static_envs]
CLAUDE_HOME = "{home}/.claude"
"#;

    let registry =
        super::registry_from_sources(&[HarnessSource::manifest("canon.toml", manifest)]).unwrap();

    let spec = registry.find("canon-test").unwrap();
    assert_eq!(
        spec.target_runtime, "Claude Code",
        "target_runtime should be canonicalized to runtime display name, not the binary alias used to look it up",
    );
}

#[test]
fn identical_user_copy_of_builtin_harness_loads_without_changing_count() {
    use crate::harnesses::builtin::BUILTIN_MANIFESTS;

    let (_, builtin_content) = BUILTIN_MANIFESTS[0];

    let registry = super::registry_from_sources(&[
        HarnessSource::builtins(),
        HarnessSource::manifest("identical.toml", builtin_content.to_string()),
    ])
    .unwrap();

    let builtin_only = super::builtin_registry().unwrap();
    assert_eq!(
        registry.specs().len(),
        builtin_only.specs().len(),
        "identical override replaces builtin, count unchanged",
    );
}

#[test]
fn harness_targeting_old_display_loads_after_binary_route_runtime_override() {
    use crate::runtimes::registry::dynamic::{RuntimeRegistry, RuntimeSource};

    let user_runtime = r#"
schema_version = 1
name = "Aaa Claude"
binary_names = ["claude"]
version_arg = "--version"

[config_locator]
kind = "env-or-home"
env = ""
home_relative = ".claude"

[auth_login]
kind = "unsupported"

[isolation]
subdir = "aaa-claude"
spoof_home = true
"#;

    let runtimes = RuntimeRegistry::from_sources(&[
        RuntimeSource::builtins(),
        RuntimeSource::manifest("aaa-claude.toml", user_runtime),
    ])
    .unwrap();

    let harness = r#"
schema_version = 1
id = "post-override"
display_name = "Test"
target_runtime = "Claude Code"
detect_binaries = ["post-override"]

[package]
kind = "npm-global"
package = "post-override-pkg"

[isolation]
spoof_home = true
home_subdirs = []

[isolation.static_envs]
CLAUDE_HOME = "{home}/.claude"
"#;

    let registry = HarnessRegistry::from_sources(
        &[HarnessSource::manifest("post-override.toml", harness)],
        &runtimes,
    )
    .unwrap();

    let spec = registry.find("post-override").unwrap();
    assert_eq!(
        spec.target_runtime, "Aaa Claude",
        "target_runtime canonicalizes via preserved route - harness referencing old 'Claude Code' resolves to overridden user runtime",
    );
}
