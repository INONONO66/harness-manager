use super::{builtin_specs, BUILTIN_MANIFESTS};
use crate::harnesses::manifest::ManifestPackageSpec;
use std::collections::HashSet;

#[test]
fn builtin_manifests_parse_all_indexed_entries() {
    // Given: the bundled harness manifests.
    let specs = builtin_specs().expect("builtins parse");

    // When: the parsed IDs are counted for uniqueness.
    let unique_ids: HashSet<&str> = specs.iter().map(|spec| spec.id.as_str()).collect();

    // Then: every bundled manifest is represented exactly once.
    assert_eq!(specs.len(), BUILTIN_MANIFESTS.len());
    assert_eq!(unique_ids.len(), specs.len());
}

#[test]
fn builtin_manifests_have_usable_package_strategies() {
    // Given: the bundled harness manifests.
    let specs = builtin_specs().expect("builtins parse");

    // When: package strategies are inspected.
    let all_strategies_have_payload = specs.iter().all(|spec| match &spec.package {
        ManifestPackageSpec::NpmGlobal { package, .. }
        | ManifestPackageSpec::NpmIsolated { package, .. }
        | ManifestPackageSpec::PythonTool { package, .. } => !package.is_empty(),
        ManifestPackageSpec::NpxInstaller { package, args, .. }
        | ManifestPackageSpec::BunxInstaller { package, args, .. } => {
            !package.is_empty() && !args.iter().any(String::is_empty)
        }
        ManifestPackageSpec::Manual { instructions, .. } => !instructions.is_empty(),
    });

    // Then: each bundled package strategy carries the command payload it needs.
    assert!(all_strategies_have_payload);
}
