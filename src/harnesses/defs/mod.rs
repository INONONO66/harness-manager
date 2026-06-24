//! Native harness definitions — one module (folder) per harness, mirroring
//! `crate::runtimes::defs`. Each harness module exposes `record() -> HarnessSpec`
//! assembled from its own `package` (install/remove) and `isolation` submodules.
//! Injection is inherited from each harness's target runtime. Add a harness =
//! add a folder + one line in `all()`.

use crate::harnesses::spec::HarnessSpec;

mod common;

pub mod gstack;
pub mod gstack_claude;
pub mod lazycodex;
pub mod omc;
pub mod omo;
pub mod omx;
pub mod ouroboros;
pub mod superpowers;
pub mod superpowers_claude;

pub fn all() -> Vec<HarnessSpec> {
    vec![
        lazycodex::record(),
        omx::record(),
        superpowers::record(),
        gstack::record(),
        ouroboros::record(),
        gstack_claude::record(),
        superpowers_claude::record(),
        omo::record(),
        omc::record(),
    ]
}

#[cfg(test)]
mod tests {
    use super::all;

    #[test]
    fn all_returns_exactly_9_records() {
        assert_eq!(all().len(), 9);
    }

    #[test]
    fn all_ids_and_aliases_are_unique() {
        let mut routes = std::collections::HashSet::new();
        for spec in all() {
            assert!(
                routes.insert(spec.id.clone()),
                "duplicate route: {}",
                spec.id
            );
            for alias in &spec.aliases {
                assert!(routes.insert(alias.clone()), "duplicate route: {alias}");
            }
        }
    }

    #[test]
    fn every_record_has_core_fields() {
        for spec in all() {
            assert!(!spec.id.is_empty(), "empty id");
            assert!(
                !spec.display_name.is_empty(),
                "empty display_name: {}",
                spec.id
            );
            assert!(
                !spec.detect_binaries.is_empty(),
                "no detect_binaries: {}",
                spec.id
            );
            assert!(
                spec.launch_binary.is_some(),
                "no launch_binary: {}",
                spec.id
            );
            assert_eq!(spec.isolation.subdir, spec.id, "subdir != id: {}", spec.id);
        }
    }

    #[test]
    fn target_runtimes_are_known_display_names() {
        for spec in all() {
            assert!(
                matches!(
                    spec.target_runtime.as_str(),
                    "Codex CLI" | "Claude Code" | "OpenCode"
                ),
                "{} has unexpected target_runtime {}",
                spec.id,
                spec.target_runtime
            );
        }
    }
}
