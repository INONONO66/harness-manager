pub mod builtin;
pub mod detect;
pub mod install;
pub mod manifest;
mod package;
pub mod registry;
pub mod types;

use types::HarnessSpec;

/// Look up a harness by id (case-insensitive).
#[allow(dead_code)]
pub fn find_harness_spec(name: &str) -> Option<&'static HarnessSpec> {
    let lower = name.to_lowercase();
    registry::HARNESSES.iter().find(|h| h.id == lower)
}
