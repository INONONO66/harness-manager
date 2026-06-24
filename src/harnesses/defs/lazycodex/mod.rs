//! lazycodex — Codex CLI wrapped by the `lazycodex-ai` installer.
//!
//! Injection is inherited from the target runtime (Codex CLI); harnesses
//! declare none of their own. `target_runtime_shared_state` is filled by the
//! registry from the target runtime record.

use crate::harnesses::spec::HarnessSpec;

mod isolation;
mod package;

pub fn record() -> HarnessSpec {
    HarnessSpec {
        id: "lazycodex".to_string(),
        aliases: vec!["lc".to_string()],
        display_name: "lazycodex".to_string(),
        target_runtime: "Codex CLI".to_string(),
        target_runtime_shared_state: None,
        package: package::package(),
        detect_binaries: vec!["codex".to_string()],
        launch_binary: Some("codex".to_string()),
        launch_args: vec![],
        isolation: isolation::isolation(),
    }
}
