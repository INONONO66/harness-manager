//! omc install/remove strategy (npm isolated).

use crate::harnesses::spec::{PackageSpec, SelfUpdatePolicy};

pub(super) fn package() -> PackageSpec {
    PackageSpec::NpmIsolated {
        package: "oh-my-claude-sisyphus".to_string(),
        self_update: Some(SelfUpdatePolicy::SuppressedByEnv),
    }
}
