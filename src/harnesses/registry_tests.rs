use anyhow::Result;

pub(super) use super::*;
use crate::runtimes::registry::RuntimeRegistry;

#[path = "registry_tests/hardcoding.rs"]
mod hardcoding;

pub(super) fn test_runtimes() -> RuntimeRegistry {
    RuntimeRegistry::builtin_only().expect("builtin runtimes load")
}

pub(super) fn builtin_registry() -> Result<HarnessRegistry> {
    let runtimes = test_runtimes();
    HarnessRegistry::builtin_only(&runtimes)
}

#[test]
fn registry_loads_all_native_harnesses() {
    let registry = builtin_registry().expect("registry loads");
    assert_eq!(registry.specs().len(), 9, "expected 9 native harnesses");
}

#[test]
fn registry_resolves_shared_state_from_target_runtime() {
    let registry = builtin_registry().expect("registry loads");
    // Every harness inherits its target runtime's shared-state plan.
    for spec in registry.specs() {
        assert!(
            spec.target_runtime_shared_state.is_some(),
            "harness '{}' did not inherit shared_state from {}",
            spec.id,
            spec.target_runtime
        );
    }
}

#[test]
fn find_resolves_ids_and_aliases_case_insensitively() {
    let registry = builtin_registry().expect("registry loads");
    assert_eq!(
        registry.find("lazycodex").map(|s| s.id.as_str()),
        Some("lazycodex")
    );
    assert_eq!(
        registry.find("LC").map(|s| s.id.as_str()),
        Some("lazycodex")
    );
    assert_eq!(
        registry.find("gsc").map(|s| s.id.as_str()),
        Some("gstack-claude")
    );
    assert!(registry.find("does-not-exist").is_none());
}
