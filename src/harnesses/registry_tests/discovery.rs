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

