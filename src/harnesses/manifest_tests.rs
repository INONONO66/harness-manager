pub(super) use super::parse_toml;

#[path = "manifest_tests/docs_and_fixtures.rs"]
mod docs_and_fixtures;
#[path = "manifest_tests/parsing.rs"]
mod parsing;
#[path = "manifest_tests/runtime_tokens.rs"]
mod runtime_tokens;
#[path = "manifest_tests/validation_cases.rs"]
mod validation_cases;

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
