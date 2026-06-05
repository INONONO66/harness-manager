use std::fs;
use std::path::{Path, PathBuf};

use super::*;
use crate::harnesses::manifest::ManifestPackageSpec;

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

fn package_name(package: &ManifestPackageSpec) -> Option<&str> {
    match package {
        ManifestPackageSpec::NpmGlobal { package }
        | ManifestPackageSpec::NpxInstaller { package, .. }
        | ManifestPackageSpec::BunxInstaller { package, .. }
        | ManifestPackageSpec::PythonTool { package } => Some(package),
        ManifestPackageSpec::Manual { .. } => None,
    }
}

#[test]
fn core_rust_sources_do_not_hardcode_builtin_harnesses() {
    let registry = HarnessRegistry::builtin_only().unwrap();
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
    let mut sources = Vec::new();
    rust_sources_under(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut sources,
    );
    sources.sort();

    let mut offenders = Vec::new();
    for source in sources {
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
        "builtin harness-specific strings must live in manifests outside core:\n{}",
        offenders.join("\n")
    );
}
