//! ouroboros install/remove strategy (python tool).

use crate::harnesses::spec::{PackageSpec, SelfUpdatePolicy};

pub(super) fn package() -> PackageSpec {
    PackageSpec::PythonTool {
        package: "ouroboros-ai[claude]".to_string(),
        self_update: Some(SelfUpdatePolicy::ManagedByHm),
    }
}
