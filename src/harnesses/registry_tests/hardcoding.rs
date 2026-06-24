use std::fs;
use std::path::{Path, PathBuf};

use crate::harnesses::spec::PackageSpec;

fn rust_sources_under(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            rust_sources_under(&path, out);
        } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn package_name(package: &PackageSpec) -> Option<&str> {
    match package {
        PackageSpec::NpmGlobal { package, .. }
        | PackageSpec::NpmIsolated { package, .. }
        | PackageSpec::NpxInstaller { package, .. }
        | PackageSpec::BunxInstaller { package, .. }
        | PackageSpec::PythonTool { package, .. } => Some(package),
        PackageSpec::Manual { .. }
        | PackageSpec::Custom { .. }
        | PackageSpec::GitWorktree { .. } => None,
    }
}

/// The harness engine (install/detect/launch/registry/…) must stay data-driven:
/// no builtin harness id, display name, or package name may be hardcoded outside
/// the declarative definitions under `src/harnesses/defs/`. This is the native
/// equivalent of the old "harness strings live in TOML, not core" invariant —
/// the strings now live in Rust, but only inside the per-harness `defs` modules.
#[test]
fn engine_sources_do_not_hardcode_builtin_harnesses() {
    let registry = super::builtin_registry().unwrap();
    let forbidden_terms: Vec<&str> = registry
        .specs()
        .iter()
        .flat_map(|spec| {
            [
                Some(spec.id.as_str()),
                Some(spec.display_name.as_str()),
                package_name(&spec.package),
            ]
        })
        .flatten()
        .collect();

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let defs_root = manifest_dir.join("src").join("harnesses").join("defs");
    let mut sources = Vec::new();
    rust_sources_under(&manifest_dir.join("src"), &mut sources);
    sources.sort();

    let mut offenders = Vec::new();
    for source in sources {
        // The declarative defs are the sanctioned home for harness data.
        if source.starts_with(&defs_root) {
            continue;
        }
        let file_name = source.file_name().and_then(|value| value.to_str());
        if matches!(file_name, Some("tests.rs"))
            || file_name.is_some_and(|name| name.ends_with("_tests.rs"))
            || source.components().any(|component| {
                component
                    .as_os_str()
                    .to_str()
                    .is_some_and(|value| value.ends_with("_tests"))
            })
        {
            continue;
        }
        let content = fs::read_to_string(&source).unwrap();
        for term in &forbidden_terms {
            if content.contains(term) {
                offenders.push(format!("{} contains {term}", source.display()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "builtin harness-specific strings must live in src/harnesses/defs, not the engine:\n{}",
        offenders.join("\n")
    );
}
