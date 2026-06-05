use super::*;
use crate::runtimes::registry::RUNTIMES;
use std::collections::HashSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

struct EnvRestore {
    home: Option<OsString>,
    xdg_config_home: Option<OsString>,
}

impl EnvRestore {
    fn capture() -> Self {
        Self {
            home: std::env::var_os("HOME"),
            xdg_config_home: std::env::var_os("XDG_CONFIG_HOME"),
        }
    }
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        if let Some(value) = &self.home {
            std::env::set_var("HOME", value);
        } else {
            std::env::remove_var("HOME");
        }
        if let Some(value) = &self.xdg_config_home {
            std::env::set_var("XDG_CONFIG_HOME", value);
        } else {
            std::env::remove_var("XDG_CONFIG_HOME");
        }
    }
}

const FORBIDDEN_CORE_HARNESS_TERMS: &[&str] = &[
    concat!("o", "mx"),
    concat!("o", "mc"),
    concat!("o", "mo"),
    concat!("lazy", "codex"),
    concat!("ouro", "boros"),
    concat!("oh", "-my-", "codex"),
    concat!("oh", "-my-", "claude"),
    concat!("oh", "-my-", "openagent"),
    concat!("lazy", "codex", "-ai"),
    concat!("ouro", "boros", "-ai"),
];

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

fn rust_sources_under(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            rust_sources_under(&path, out);
        } else if path.extension().and_then(|value| value.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

#[test]
fn core_rust_sources_do_not_hardcode_builtin_harnesses() {
    let mut sources = Vec::new();
    rust_sources_under(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut sources,
    );
    sources.sort();

    let mut offenders = Vec::new();
    for source in sources {
        let file_name = source.file_name().and_then(|value| value.to_str());
        if matches!(file_name, Some("tests.rs") | Some("registry_tests.rs"))
            || file_name.is_some_and(|name| name.ends_with("_tests.rs"))
        {
            continue;
        }
        let content = fs::read_to_string(&source).unwrap();
        for term in FORBIDDEN_CORE_HARNESS_TERMS {
            if content.contains(term) {
                offenders.push(format!("{} contains {term}", source.display()));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "builtin harness-specific strings must live in manifests outside core:\n{}",
        offenders.join("\n")
    );
}

#[test]
fn all_ids_unique() {
    let registry = HarnessRegistry::builtin_only().unwrap();
    let mut seen: HashSet<&str> = HashSet::new();
    for h in registry.specs() {
        assert!(seen.insert(h.id.as_str()), "duplicate harness id: {}", h.id);
    }
}

#[test]
fn all_target_runtimes_valid() {
    let registry = HarnessRegistry::builtin_only().unwrap();
    for h in registry.specs() {
        assert!(
            RUNTIMES.iter().any(|r| r.name == h.target_runtime),
            "harness {} targets unknown runtime {}",
            h.id,
            h.target_runtime,
        );
    }
}

#[test]
fn registry_find_is_case_insensitive() {
    let registry = HarnessRegistry::from_sources(&[HarnessSource::manifest(
        "lookup.toml",
        demo_manifest("lookup-plugin"),
    )])
    .unwrap();
    assert!(registry.find("lookup-plugin").is_some());
    assert!(
        registry.find("LOOKUP-PLUGIN").is_some(),
        "lookup is case-insensitive"
    );
    assert!(registry.find("nope").is_none());
}

#[test]
fn harness_count_is_five() {
    let registry = HarnessRegistry::builtin_only().unwrap();
    assert_eq!(registry.specs().len(), 5);
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
    assert!(registry.specs().len() > 1);
}

#[test]
fn registry_rejects_duplicate_builtin_id() {
    let builtin = HarnessRegistry::builtin_only().unwrap();
    let duplicate_id = builtin.specs()[0].id.as_str();
    let err = HarnessRegistry::from_sources(&[
        HarnessSource::builtins(),
        HarnessSource::manifest("user/duplicate.toml", demo_manifest(duplicate_id)),
    ])
    .unwrap_err();

    assert!(
        err.to_string()
            .contains(&format!("duplicate harness id '{duplicate_id}'")),
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
fn registry_load_from_env_does_not_fallback_to_process_xdg() {
    let _guard = ENV_LOCK.lock().unwrap();
    let temp = tempfile::tempdir().unwrap();
    let process_home = temp.path().join("process-home");
    for root in [
        process_home.join(".config"),
        process_home.join("Library").join("Application Support"),
    ] {
        let harness_dir = root.join("hm").join("harnesses.d");
        fs::create_dir_all(&harness_dir).unwrap();
        fs::write(harness_dir.join("leaked.toml"), demo_manifest("leaked")).unwrap();
    }

    let _restore = EnvRestore::capture();
    std::env::set_var("HOME", &process_home);
    std::env::remove_var("XDG_CONFIG_HOME");
    let registry = HarnessRegistry::load_from_env(&HarnessDiscoveryEnv {
        xdg_config_home: None,
        xdg_data_home: None,
        home: Some(temp.path().join("provided-home")),
    })
    .unwrap();

    assert!(
        registry.find("leaked").is_none(),
        "load_from_env must use the provided discovery env only"
    );
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
