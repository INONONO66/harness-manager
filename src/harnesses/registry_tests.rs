use super::super::find_harness_spec;
use super::dynamic::{HarnessDiscoveryEnv, HarnessRegistry, HarnessSource};
use super::*;
use crate::runtimes::registry::RUNTIMES;
use std::collections::HashSet;
use std::fs;

fn demo_manifest(id: &str) -> String {
    format!(
        r#"
schema_version = 1
id = "{id}"
display_name = "{id}"
target_runtime = "Codex CLI"
detect_binaries = ["{id}"]

[package]
kind = "npm-global"
package = "{id}-package"

[isolation]
spoof_home = true
home_subdirs = [".codex"]

[isolation.static_envs]
CODEX_HOME = "{{home}}/.codex"
"#
    )
}

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

#[test]
fn registry_loads_user_manifest_from_xdg_config_home() {
    let temp = tempfile::tempdir().unwrap();
    let harness_dir = temp.path().join("config").join("hm").join("harnesses.d");
    fs::create_dir_all(&harness_dir).unwrap();
    fs::write(harness_dir.join("demo.toml"), demo_manifest("demo")).unwrap();

    let registry = HarnessRegistry::load_from_env(&HarnessDiscoveryEnv {
        xdg_config_home: Some(temp.path().join("config")),
        xdg_data_home: Some(temp.path().join("data")),
        home: Some(temp.path().join("home")),
    })
    .unwrap();

    assert!(registry.find("demo").is_some());
    assert!(registry.find("omx").is_some());
}

#[test]
fn registry_rejects_duplicate_builtin_id() {
    let err = HarnessRegistry::from_sources(&[
        HarnessSource::builtins(),
        HarnessSource::manifest("user/omx.toml", demo_manifest("omx")),
    ])
    .unwrap_err();

    assert!(
        err.to_string().contains("duplicate harness id 'omx'"),
        "expected duplicate id error, got: {err:#}"
    );
}

#[test]
fn registry_from_sources_does_not_read_process_env() {
    let first = HarnessRegistry::from_sources(&[HarnessSource::manifest(
        "first.toml",
        demo_manifest("first"),
    )])
    .unwrap();
    let second = HarnessRegistry::from_sources(&[HarnessSource::manifest(
        "second.toml",
        demo_manifest("second"),
    )])
    .unwrap();

    assert!(first.find("first").is_some());
    assert!(first.find("second").is_none());
    assert!(second.find("second").is_some());
    assert!(second.find("first").is_none());
}

#[test]
fn registry_source_order_is_deterministic() {
    let registry = HarnessRegistry::from_sources(&[
        HarnessSource::manifest("z.toml", demo_manifest("zeta")),
        HarnessSource::manifest("a.toml", demo_manifest("alpha")),
    ])
    .unwrap();

    let ids: Vec<&str> = registry
        .specs()
        .iter()
        .map(|spec| spec.id.as_str())
        .collect();

    assert_eq!(ids, ["alpha", "zeta"]);
}

#[cfg(unix)]
#[test]
fn registry_rejects_symlink_manifest_escape() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let config_root = temp.path().join("config");
    let harness_dir = config_root.join("hm").join("harnesses.d");
    let outside = temp.path().join("outside.toml");
    fs::create_dir_all(&harness_dir).unwrap();
    fs::write(&outside, demo_manifest("escape")).unwrap();
    symlink(&outside, harness_dir.join("escape.toml")).unwrap();

    let err = HarnessRegistry::load_from_env(&HarnessDiscoveryEnv {
        xdg_config_home: Some(config_root),
        xdg_data_home: Some(temp.path().join("data")),
        home: Some(temp.path().join("home")),
    })
    .unwrap_err();

    assert!(
        err.to_string().contains("symlink"),
        "expected symlink rejection, got: {err:#}"
    );
}
