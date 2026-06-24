//! gstack — Codex CLI with gstack skills (git-worktree install).
//!
//! Injection is inherited from the target runtime (Codex CLI).

use crate::harnesses::spec::HarnessSpec;

mod isolation;
mod package;

pub fn record() -> HarnessSpec {
    HarnessSpec {
        id: "gstack".to_string(),
        aliases: vec!["gs".to_string()],
        display_name: "gstack".to_string(),
        target_runtime: "Codex CLI".to_string(),
        target_runtime_shared_state: None,
        package: package::package(),
        detect_binaries: vec!["codex".to_string()],
        launch_binary: Some("codex".to_string()),
        launch_args: vec![],
        isolation: isolation::isolation(),
    }
}
