//! superpowers install/update/remove strategy (Codex plugin commands).

use crate::harnesses::spec::{PackageCommandTemplate, PackageSpec, SelfUpdatePolicy};

pub(super) fn package() -> PackageSpec {
    PackageSpec::Custom {
        install: PackageCommandTemplate {
            argv: vec![
                "codex".to_string(),
                "plugin".to_string(),
                "add".to_string(),
                "superpowers".to_string(),
            ],
        },
        update: Some(PackageCommandTemplate {
            argv: vec![
                "codex".to_string(),
                "plugin".to_string(),
                "add".to_string(),
                "superpowers".to_string(),
            ],
        }),
        uninstall: Some(PackageCommandTemplate {
            argv: vec![
                "codex".to_string(),
                "plugin".to_string(),
                "remove".to_string(),
                "superpowers".to_string(),
            ],
        }),
        bin_subdir: None,
        self_update: Some(SelfUpdatePolicy::ManagedByHm),
    }
}
