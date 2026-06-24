//! lazycodex install/remove strategy.

use crate::harnesses::spec::{PackageSpec, SelfUpdatePolicy};

pub(super) fn package() -> PackageSpec {
    PackageSpec::NpxInstaller {
        package: "lazycodex-ai".to_string(),
        args: vec!["install".to_string()],
        self_update: Some(SelfUpdatePolicy::ManagedByHm),
    }
}
