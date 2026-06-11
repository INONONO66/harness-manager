use std::ffi::OsString;
use std::fs;
use std::sync::Mutex;

use super::*;

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

#[test]
fn registry_loads_user_manifest_from_xdg_config_home() {
    let temp = tempfile::tempdir().unwrap();
    let harness_dir = temp.path().join("config").join("hm").join("harnesses.d");
    fs::create_dir_all(&harness_dir).unwrap();
    fs::write(harness_dir.join("demo.toml"), demo_manifest("demo")).unwrap();

    let runtimes = super::test_runtimes();
    let registry = HarnessRegistry::load_from_env(
        &HarnessDiscoveryEnv {
            xdg_config_home: Some(temp.path().join("config")),
            xdg_data_home: Some(temp.path().join("data")),
            home: Some(temp.path().join("home")),
        },
        &runtimes,
    )
    .unwrap();

    assert!(registry.find("demo").is_some());
    assert!(registry.specs().len() > 1);
}

#[test]
fn registry_from_sources_does_not_read_process_env() {
    let first = super::registry_from_sources(&[HarnessSource::manifest(
        "first.toml",
        demo_manifest("first"),
    )])
    .unwrap();
    let second = super::registry_from_sources(&[HarnessSource::manifest(
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
    let runtimes = super::test_runtimes();
    let registry = HarnessRegistry::load_from_env(
        &HarnessDiscoveryEnv {
            xdg_config_home: None,
            xdg_data_home: None,
            home: Some(temp.path().join("provided-home")),
        },
        &runtimes,
    )
    .unwrap();

    assert!(
        registry.find("leaked").is_none(),
        "load_from_env must use the provided discovery env only"
    );
}

#[test]
fn registry_source_order_is_deterministic() {
    let registry = super::registry_from_sources(&[
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

    let runtimes = super::test_runtimes();
    let err = HarnessRegistry::load_from_env(
        &HarnessDiscoveryEnv {
            xdg_config_home: Some(config_root),
            xdg_data_home: Some(temp.path().join("data")),
            home: Some(temp.path().join("home")),
        },
        &runtimes,
    )
    .unwrap_err();

    assert!(
        err.to_string().contains("symlink"),
        "expected symlink rejection, got: {err:#}"
    );
}

#[cfg(unix)]
#[test]
fn registry_rejects_oversized_harness_manifest_before_reading_contents() {
    use std::os::unix::fs::PermissionsExt;

    // Given: an oversized harness manifest that cannot be read by this process.
    let temp = tempfile::tempdir().unwrap();
    let config_root = temp.path().join("config");
    let harness_dir = config_root.join("hm").join("harnesses.d");
    let oversized = harness_dir.join("oversized.toml");
    fs::create_dir_all(&harness_dir).unwrap();
    fs::write(&oversized, "x".repeat(70 * 1024)).unwrap();
    let mut permissions = fs::metadata(&oversized).unwrap().permissions();
    permissions.set_mode(0o000);
    fs::set_permissions(&oversized, permissions).unwrap();

    // When: the registry discovers that manifest.
    let runtimes = super::test_runtimes();
    let err = HarnessRegistry::load_from_env(
        &HarnessDiscoveryEnv {
            xdg_config_home: Some(config_root),
            xdg_data_home: Some(temp.path().join("data")),
            home: Some(temp.path().join("home")),
        },
        &runtimes,
    )
    .unwrap_err();

    let mut permissions = fs::metadata(&oversized).unwrap().permissions();
    permissions.set_mode(0o600);
    fs::set_permissions(&oversized, permissions).unwrap();

    // Then: it rejects by metadata size before attempting to read contents.
    assert!(
        err.to_string().contains("exceeds 64 KiB"),
        "expected pre-read size rejection, got: {err:#}"
    );
}
