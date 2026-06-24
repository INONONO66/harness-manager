//! omc — oh-my-claudecode on top of Claude Code.
//!
//! Injection is inherited from the target runtime (Claude Code), which also
//! makes this harness SpoofHome.

use crate::harnesses::spec::HarnessSpec;

mod isolation;
mod package;

pub fn record() -> HarnessSpec {
    HarnessSpec {
        id: "omc".to_string(),
        aliases: vec![],
        display_name: "oh-my-claudecode".to_string(),
        target_runtime: "Claude Code".to_string(),
        target_runtime_shared_state: None,
        package: package::package(),
        detect_binaries: vec!["omc".to_string()],
        launch_binary: Some("omc".to_string()),
        launch_args: vec![],
        isolation: isolation::isolation(),
    }
}
