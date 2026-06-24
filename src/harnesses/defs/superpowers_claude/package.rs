//! superpowers-claude install/update/remove strategy (Claude plugin commands).

use crate::harnesses::spec::{PackageCommandTemplate, PackageSpec, SelfUpdatePolicy};

pub(super) fn package() -> PackageSpec {
    PackageSpec::Custom {
        install: PackageCommandTemplate {
            argv: vec![
                "claude".to_string(),
                "plugin".to_string(),
                "install".to_string(),
                "--scope".to_string(),
                "user".to_string(),
                "superpowers@claude-plugins-official".to_string(),
            ],
        },
        update: Some(PackageCommandTemplate {
            argv: vec![
                "claude".to_string(),
                "plugin".to_string(),
                "update".to_string(),
                "--scope".to_string(),
                "user".to_string(),
                "superpowers".to_string(),
            ],
        }),
        uninstall: Some(PackageCommandTemplate {
            argv: vec![
                "claude".to_string(),
                "plugin".to_string(),
                "uninstall".to_string(),
                "--scope".to_string(),
                "user".to_string(),
                "--yes".to_string(),
                "superpowers".to_string(),
            ],
        }),
        bin_subdir: None,
        self_update: Some(SelfUpdatePolicy::ManagedByHm),
    }
}
