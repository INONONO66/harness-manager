//! omo — oh-my-openagent on top of OpenCode.
//!
//! Injection is inherited from the target runtime (OpenCode).

use crate::harnesses::spec::HarnessSpec;

mod isolation;
mod package;

pub fn record() -> HarnessSpec {
    HarnessSpec {
        id: "omo".to_string(),
        aliases: vec![],
        display_name: "oh-my-openagent".to_string(),
        target_runtime: "OpenCode".to_string(),
        target_runtime_shared_state: None,
        package: package::package(),
        detect_binaries: vec!["opencode".to_string()],
        launch_binary: Some("opencode".to_string()),
        launch_args: vec![],
        isolation: isolation::isolation(),
    }
}
