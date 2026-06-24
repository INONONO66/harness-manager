//! gstack-claude — Claude Code with gstack skills (git-worktree install).
//!
//! Injection is inherited from the target runtime (Claude Code), which also
//! makes this harness SpoofHome.

use crate::harnesses::spec::HarnessSpec;

mod isolation;
mod package;

pub fn record() -> HarnessSpec {
    HarnessSpec {
        id: "gstack-claude".to_string(),
        aliases: vec!["gstack-cc".to_string(), "gsc".to_string()],
        display_name: "gstack (Claude Code)".to_string(),
        target_runtime: "Claude Code".to_string(),
        target_runtime_shared_state: None,
        package: package::package(),
        detect_binaries: vec!["claude".to_string()],
        launch_binary: Some("claude".to_string()),
        launch_args: vec![],
        isolation: isolation::isolation(),
    }
}
