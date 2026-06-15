use super::*;
use crate::harnesses::manifest::PackageCommandTemplate;
use crate::harnesses::types::{HarnessSpec, PackageSpec};
use crate::isolation::spec::IsolationPlan;

fn test_runtimes() -> crate::runtimes::registry::RuntimeRegistry {
    crate::runtimes::registry::RuntimeRegistry::builtin_only().unwrap()
}

fn empty_iso(subdir: &str) -> IsolationPlan {
    IsolationPlan {
        subdir: subdir.to_string(),
        runtime_subdir: subdir.to_string(),
        spoof_home: false,
        home_subdirs: Vec::new(),
        static_envs: Vec::new(),
        seed_files: Vec::new(),
        caveat: None,
    }
}

fn unique_subdir(label: &str) -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    format!("hm-detect-test-{}-{}-{nanos}", label, std::process::id())
}

#[path = "detect_tests/cache.rs"]
mod cache;
#[path = "detect_tests/core.rs"]
mod core;
#[path = "detect_tests/installer.rs"]
mod installer;
