//! gstack-claude install/update strategy (git-worktree).

use crate::harnesses::spec::{PackageCommandTemplate, PackageSpec, SelfUpdatePolicy};

pub(super) fn package() -> PackageSpec {
    PackageSpec::GitWorktree {
        repository: "https://github.com/garrytan/gstack".to_string(),
        setup: PackageCommandTemplate {
            argv: vec![
                "setup".to_string(),
                "--host".to_string(),
                "claude".to_string(),
                "--no-prefix".to_string(),
                "--quiet".to_string(),
            ],
        },
        update: Some(PackageCommandTemplate {
            argv: vec![
                "setup".to_string(),
                "--host".to_string(),
                "claude".to_string(),
                "--no-prefix".to_string(),
                "--quiet".to_string(),
            ],
        }),
        self_update: Some(SelfUpdatePolicy::ManagedByHm),
    }
}
