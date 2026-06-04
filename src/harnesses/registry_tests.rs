use super::super::find_harness_spec;
use super::*;
use crate::runtimes::registry::RUNTIMES;
use std::collections::HashSet;

#[test]
fn all_ids_unique() {
    let mut seen: HashSet<&str> = HashSet::new();
    for h in HARNESSES {
        assert!(seen.insert(h.id), "duplicate harness id: {}", h.id);
    }
}

#[test]
fn all_target_runtimes_valid() {
    for h in HARNESSES {
        assert!(
            RUNTIMES.iter().any(|r| r.name == h.target_runtime),
            "harness {} targets unknown runtime {}",
            h.id,
            h.target_runtime,
        );
    }
}

#[test]
fn find_harness_spec_works() {
    assert!(find_harness_spec("omx").is_some());
    assert!(
        find_harness_spec("OMX").is_some(),
        "lookup is case-insensitive"
    );
    assert!(find_harness_spec("nope").is_none());
}

#[test]
fn harness_count_is_five() {
    assert_eq!(HARNESSES.len(), 5);
}

#[test]
fn omo_declares_opencode_config_dir() {
    let omo = find_harness_spec("omo").unwrap();

    assert!(
        omo.isolation
            .static_envs
            .iter()
            .any(|(k, v)| *k == "OPENCODE_CONFIG_DIR" && *v == "{home}/.config/opencode"),
        "omo must route documented OpenCode config writes into its isolated home"
    );
}

#[test]
fn omx_declares_state_roots() {
    let omx = find_harness_spec("omx").unwrap();

    for key in ["OMX_ROOT", "OMX_STATE_ROOT", "OMX_TEAM_STATE_ROOT"] {
        assert!(
            omx.isolation.static_envs.iter().any(|(k, _)| *k == key),
            "omx must isolate {} for concurrent oh-my-codex sessions",
            key
        );
    }
}
