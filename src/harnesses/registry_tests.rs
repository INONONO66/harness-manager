use anyhow::Result;

pub(super) use super::*;
use crate::runtimes::registry::RuntimeRegistry;

#[path = "registry_tests/discovery.rs"]
mod discovery;
#[path = "registry_tests/hardcoding.rs"]
mod hardcoding;
#[path = "registry_tests/sources.rs"]
mod sources;

pub(super) fn test_runtimes() -> RuntimeRegistry {
    RuntimeRegistry::builtin_only().expect("builtin runtimes load")
}

pub(super) fn builtin_registry() -> Result<HarnessRegistry> {
    let runtimes = test_runtimes();
    HarnessRegistry::builtin_only(&runtimes)
}

pub(super) fn registry_from_sources(sources: &[HarnessSource]) -> Result<HarnessRegistry> {
    let runtimes = test_runtimes();
    HarnessRegistry::from_sources(sources, &runtimes)
}

pub(super) fn demo_manifest(id: &str) -> String {
    format!(
        r#"
schema_version = 1
id = "{id}"
display_name = "{id}"
target_runtime = "Codex CLI"
detect_binaries = ["{id}"]

[package]
kind = "npm-global"
package = "{id}-package"

[isolation]
spoof_home = true
home_subdirs = [".codex"]

[isolation.static_envs]
CODEX_HOME = "{{home}}/.codex"
"#
    )
}
