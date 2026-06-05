use super::builtin_specs;
use crate::harnesses::manifest::ManifestPackageSpec;

#[test]
fn builtin_manifests_parse_all_five() {
    // Given: the bundled harness manifests.
    let mut ids: Vec<String> = builtin_specs()
        .expect("builtins parse")
        .into_iter()
        .map(|spec| spec.id)
        .collect();

    // When: the parsed IDs are sorted.
    ids.sort();

    // Then: every existing builtin harness is represented exactly once.
    assert_eq!(ids, ["lazycodex", "omc", "omo", "omx", "ouroboros"]);
}

#[test]
fn builtin_manifests_preserve_existing_package_strategies() {
    // Given: the bundled harness manifests.
    let specs = builtin_specs().expect("builtins parse");

    // When: package strategies are inspected by ID.
    let package_for = |id: &str| {
        specs
            .iter()
            .find(|spec| spec.id == id)
            .map(|spec| &spec.package)
            .expect("builtin id exists")
    };

    // Then: strategies match the legacy static registry.
    assert!(matches!(
        package_for("lazycodex"),
        ManifestPackageSpec::NpxInstaller { package, args }
            if package == "lazycodex-ai" && args == &["install"]
    ));
    assert!(matches!(
        package_for("omo"),
        ManifestPackageSpec::BunxInstaller { package, args }
            if package == "oh-my-openagent" && args == &["install"]
    ));
    assert!(matches!(
        package_for("omx"),
        ManifestPackageSpec::NpmGlobal { package } if package == "oh-my-codex"
    ));
    assert!(matches!(
        package_for("omc"),
        ManifestPackageSpec::NpmGlobal { package } if package == "oh-my-claude-sisyphus"
    ));
    assert!(matches!(
        package_for("ouroboros"),
        ManifestPackageSpec::PythonTool { package } if package == "ouroboros-ai"
    ));
}
