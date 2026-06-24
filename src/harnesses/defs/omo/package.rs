//! omo install/remove strategy (bunx installer).

use crate::harnesses::spec::{PackageSpec, SelfUpdatePolicy};

pub(super) fn package() -> PackageSpec {
    PackageSpec::BunxInstaller {
        package: "oh-my-openagent".to_string(),
        args: vec![
            "install".to_string(),
            "--no-tui".to_string(),
            "--platform=opencode".to_string(),
            "--claude=no".to_string(),
            "--openai=no".to_string(),
            "--gemini=no".to_string(),
            "--copilot=no".to_string(),
            "--skip-auth".to_string(),
        ],
        self_update: Some(SelfUpdatePolicy::SuppressedByEnv),
    }
}
