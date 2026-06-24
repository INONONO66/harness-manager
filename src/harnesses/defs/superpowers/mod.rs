//! superpowers — Codex CLI with the Superpowers plugin.
//!
//! Injection is inherited from the target runtime (Codex CLI).

use crate::harnesses::spec::HarnessSpec;

mod isolation;
mod package;

pub fn record() -> HarnessSpec {
    HarnessSpec {
        id: "superpowers".to_string(),
        aliases: vec!["sp".to_string()],
        display_name: "superpowers".to_string(),
        target_runtime: "Codex CLI".to_string(),
        target_runtime_shared_state: None,
        package: package::package(),
        detect_binaries: vec!["codex".to_string()],
        launch_binary: Some("codex".to_string()),
        launch_args: vec![],
        isolation: isolation::isolation(),
    }
}
