pub(super) use super::*;

#[path = "registry_tests/discovery.rs"]
mod discovery;
#[path = "registry_tests/hardcoding.rs"]
mod hardcoding;
#[path = "registry_tests/sources.rs"]
mod sources;

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
