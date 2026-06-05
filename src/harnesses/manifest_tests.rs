use anyhow::Result;

use super::{parse_toml as parse_toml_inner, ManifestHarnessSpec};
use crate::runtimes::registry::RuntimeRegistry;

#[path = "manifest_tests/docs_and_fixtures.rs"]
mod docs_and_fixtures;
#[path = "manifest_tests/parsing.rs"]
mod parsing;
#[path = "manifest_tests/runtime_tokens.rs"]
mod runtime_tokens;
#[path = "manifest_tests/validation_cases.rs"]
mod validation_cases;

pub(super) fn parse_toml(path_label: &str, input: &str) -> Result<ManifestHarnessSpec> {
    let runtimes = RuntimeRegistry::builtin_only()?;
    parse_toml_inner(path_label, input, &runtimes)
}

pub(super) fn minimal_manifest(extra: &str) -> String {
    format!(
        r#"
schema_version = 1
id = "demo"
display_name = "Demo Harness"
target_runtime = "Codex CLI"
detect_binaries = ["demo"]

[package]
kind = "npm-global"
package = "demo-package"

[isolation]
spoof_home = true
home_subdirs = [".codex"]
static_envs = {{ CODEX_HOME = "{{home}}/.codex" }}
{extra}
"#
    )
}
