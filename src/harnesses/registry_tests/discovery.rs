use std::collections::HashSet;

use super::*;
use crate::harnesses::builtin::BUILTIN_MANIFESTS;
use crate::runtimes::registry::RUNTIMES;

#[test]
fn all_ids_unique() {
    let registry = HarnessRegistry::builtin_only().unwrap();
    let mut seen: HashSet<&str> = HashSet::new();
    for h in registry.specs() {
        assert!(seen.insert(h.id.as_str()), "duplicate harness id: {}", h.id);
    }
}

#[test]
fn all_target_runtimes_valid() {
    let registry = HarnessRegistry::builtin_only().unwrap();
    for h in registry.specs() {
        assert!(
            RUNTIMES.iter().any(|r| r.name == h.target_runtime),
            "harness {} targets unknown runtime {}",
            h.id,
            h.target_runtime,
        );
    }
}

#[test]
fn registry_find_is_case_insensitive() {
    let registry = HarnessRegistry::from_sources(&[HarnessSource::manifest(
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
        HarnessRegistry::from_sources(&[HarnessSource::manifest("alias.toml", manifest)]).unwrap();

    assert_eq!(registry.find("lp").unwrap().id, "long-plugin");
    assert_eq!(registry.find("LP").unwrap().id, "long-plugin");
}

#[test]
fn registry_rejects_duplicate_alias_route() {
    let first =
        demo_manifest("first").replace(r#"id = "first""#, "id = \"first\"\naliases = [\"dup\"]");
    let second =
        demo_manifest("second").replace(r#"id = "second""#, "id = \"second\"\naliases = [\"dup\"]");

    let err = HarnessRegistry::from_sources(&[
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

    let err = HarnessRegistry::from_sources(&[
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
    let registry = HarnessRegistry::builtin_only().unwrap();

    assert_eq!(registry.specs().len(), BUILTIN_MANIFESTS.len());
}

#[test]
fn registry_rejects_duplicate_builtin_id() {
    let builtin = HarnessRegistry::builtin_only().unwrap();
    let duplicate_id = builtin.specs()[0].id.as_str();
    let err = HarnessRegistry::from_sources(&[
        HarnessSource::builtins(),
        HarnessSource::manifest("user/duplicate.toml", demo_manifest(duplicate_id)),
    ])
    .unwrap_err();

    assert!(
        err.to_string()
            .contains(&format!("duplicate harness id '{duplicate_id}'")),
        "expected duplicate id error, got: {err:#}"
    );
}
