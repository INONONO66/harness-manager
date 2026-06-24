//! ouroboros — Codex CLI with the ouroboros workflow engine.
//!
//! Injection is inherited from the target runtime (Codex CLI).

use crate::harnesses::spec::HarnessSpec;

mod isolation;
mod package;

pub fn record() -> HarnessSpec {
    HarnessSpec {
        id: "ouroboros".to_string(),
        aliases: vec![],
        display_name: "ouroboros".to_string(),
        target_runtime: "Codex CLI".to_string(),
        target_runtime_shared_state: None,
        package: package::package(),
        detect_binaries: vec!["ouroboros".to_string()],
        launch_binary: Some("ouroboros".to_string()),
        launch_args: vec![],
        isolation: isolation::isolation(),
    }
}
