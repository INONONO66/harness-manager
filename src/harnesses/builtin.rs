#[cfg(test)]
use super::manifest::ManifestHarnessSpec;

include!(concat!(env!("OUT_DIR"), "/builtin_manifest_index.rs"));

#[cfg(test)]
pub fn builtin_specs() -> anyhow::Result<Vec<ManifestHarnessSpec>> {
    let runtimes = crate::runtimes::registry::RuntimeRegistry::builtin_only()?;
    BUILTIN_MANIFESTS
        .iter()
        .map(|(path, input)| super::manifest::parse_toml(path, input, &runtimes))
        .collect()
}

#[cfg(test)]
#[path = "builtin_tests.rs"]
mod tests;
