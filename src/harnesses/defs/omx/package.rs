//! omx install/remove strategy.

use crate::harnesses::spec::{PackageSpec, SelfUpdatePolicy};

pub(super) fn package() -> PackageSpec {
    PackageSpec::NpmIsolated {
        package: "oh-my-codex".to_string(),
        self_update: Some(SelfUpdatePolicy::SuppressedByEnv),
    }
}
