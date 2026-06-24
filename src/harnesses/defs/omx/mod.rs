//! omx — oh-my-codex on top of Codex CLI.
//!
//! Injection is inherited from the target runtime (Codex CLI).

use crate::harnesses::spec::HarnessSpec;

mod isolation;
mod package;

pub fn record() -> HarnessSpec {
    HarnessSpec {
        id: "omx".to_string(),
        aliases: vec![],
        display_name: "oh-my-codex".to_string(),
        target_runtime: "Codex CLI".to_string(),
        target_runtime_shared_state: None,
        package: package::package(),
        detect_binaries: vec!["omx".to_string()],
        launch_binary: Some("omx".to_string()),
        launch_args: vec![],
        isolation: isolation::isolation(),
    }
}
