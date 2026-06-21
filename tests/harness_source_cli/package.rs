use std::fs;
use std::process::Command;

use super::support::{demo_manifest, fake_npm};

#[test]
fn harness_install_package_with_alias_writes_manifest_and_installs() {
    // Given: only a package install recipe and a fake npm on PATH.
    let temp = tempfile::tempdir().unwrap();
    let bin = temp.path().join("bin");
    let data = temp.path().join("data");
    let config = temp.path().join("config");
    let home = temp.path().join("home");
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(home.join(".codex/sessions")).unwrap();
    fs::write(home.join(".codex/auth.json"), r#"{"token":"host"}"#).unwrap();
    fs::write(home.join(".codex/sessions/secret.jsonl"), "session").unwrap();
    let stale_iso_codex = data.join("hm/runtimes/pkg-demo/home/.codex");
    fs::create_dir_all(stale_iso_codex.join("sessions")).unwrap();
    std::os::unix::fs::symlink(
        home.join(".codex/auth.json"),
        stale_iso_codex.join("auth.json"),
    )
    .unwrap();
    std::os::unix::fs::symlink(
        home.join(".codex/sessions/secret.jsonl"),
        stale_iso_codex.join("sessions/secret.jsonl"),
    )
    .unwrap();
    let npm_log = temp.path().join("npm.log");
    fake_npm(&bin, &npm_log);

    // When: the user installs from package metadata with a chosen alias.
    let output = Command::new(env!("CARGO_BIN_EXE_hm"))
        .args([
            "harness",
            "install-package",
            "demo-package",
            "--alias",
            "pkg-demo",
            "--runtime",
            "codex",
            "--kind",
            "npm-global",
            "--binary",
            "demo",
        ])
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CONFIG_HOME", &config)
        .env("HOME", &home)
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .output()
        .unwrap();

    // Then: hm creates a discoverable manifest and runs the existing install path.
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let manifest = fs::read_to_string(data.join("hm/harnesses.d/pkg-demo.toml")).unwrap();
    assert!(manifest.contains("id = \"pkg-demo\""));
    assert!(manifest.contains("target_runtime = \"Codex CLI\""));
    assert!(manifest.contains("kind = \"npm-global\""));
    assert!(manifest.contains("self_update = \"managed-by-hm\""));
    let npm_output = fs::read_to_string(npm_log).unwrap();
    assert!(
        !npm_output.contains("auth_link_visible"),
        "package manager must not see host auth files:\n{npm_output}"
    );
    assert!(
        !npm_output.contains("session_link_visible"),
        "package manager must not see host session files:\n{npm_output}"
    );
    assert_eq!(npm_output, "install\n-g\ndemo-package\n");
}

#[test]
fn harness_install_package_uses_target_runtime_isolation_envs() {
    // Given: a package-backed OpenCode harness recipe.
    let temp = tempfile::tempdir().unwrap();
    let bin = temp.path().join("bin");
    let data = temp.path().join("data");
    let config = temp.path().join("config");
    let home = temp.path().join("home");
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(&home).unwrap();
    fake_npm(&bin, &temp.path().join("npm.log"));

    // When: the user generates the harness for OpenCode.
    let output = Command::new(env!("CARGO_BIN_EXE_hm"))
        .args([
            "harness",
            "install-package",
            "opencode-demo",
            "--alias",
            "op-demo",
            "--runtime",
            "opencode",
            "--kind",
            "npm-global",
            "--binary",
            "op-demo",
        ])
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CONFIG_HOME", &config)
        .env("HOME", &home)
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .output()
        .unwrap();

    // Then: the generated manifest inherits OpenCode's isolation envs.
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let manifest = fs::read_to_string(data.join("hm/harnesses.d/op-demo.toml")).unwrap();
    assert!(
        manifest.contains("XDG_CONFIG_HOME"),
        "manifest:\n{manifest}"
    );
    assert!(
        !manifest.contains("CODEX_HOME"),
        "OpenCode harness must not get Codex envs:\n{manifest}"
    );
}

#[test]
fn harness_install_package_rejects_alias_registered_by_plugin_source() {
    // Given: an alias already registered from a plugin source.
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("data");
    let config = temp.path().join("config");
    let home = temp.path().join("home");
    let plugin = data.join("hm/plugins/team-demo");
    fs::create_dir_all(&plugin).unwrap();
    fs::create_dir_all(&home).unwrap();
    fs::write(plugin.join("harness.toml"), demo_manifest("team-demo")).unwrap();

    // When: package generation tries to reuse the same alias.
    let output = Command::new(env!("CARGO_BIN_EXE_hm"))
        .args([
            "harness",
            "install-package",
            "demo-package",
            "--alias",
            "team-demo",
            "--runtime",
            "codex",
            "--kind",
            "npm-global",
            "--binary",
            "demo",
        ])
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CONFIG_HOME", &config)
        .env("HOME", &home)
        .output()
        .unwrap();

    // Then: hm rejects the duplicate and leaves no generated manifest behind.
    assert!(!output.status.success(), "duplicate alias should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("already registered"), "stderr:\n{stderr}");
    assert!(
        !data.join("hm/harnesses.d/team-demo.toml").exists(),
        "duplicate generated manifest must not remain"
    );
}

#[test]
fn harness_install_package_rejects_alias_registered_in_config_without_partial_manifest() {
    // Given: an alias already registered from the config discovery path.
    let temp = tempfile::tempdir().unwrap();
    let data = temp.path().join("data");
    let config = temp.path().join("config");
    let home = temp.path().join("home");
    let config_harnesses = config.join("hm/harnesses.d");
    fs::create_dir_all(&config_harnesses).unwrap();
    fs::create_dir_all(&home).unwrap();
    fs::write(
        config_harnesses.join("team-demo.toml"),
        demo_manifest("team-demo"),
    )
    .unwrap();

    // When: package generation tries to reuse the same alias.
    let output = Command::new(env!("CARGO_BIN_EXE_hm"))
        .args([
            "harness",
            "install-package",
            "demo-package",
            "--alias",
            "team-demo",
            "--runtime",
            "codex",
            "--kind",
            "npm-global",
            "--binary",
            "demo",
        ])
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CONFIG_HOME", &config)
        .env("HOME", &home)
        .output()
        .unwrap();

    // Then: hm rejects the duplicate and leaves no generated manifest behind.
    assert!(!output.status.success(), "duplicate alias should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("already registered"), "stderr:\n{stderr}");
    assert!(
        !data.join("hm/harnesses.d/team-demo.toml").exists(),
        "duplicate generated manifest must not remain"
    );
}
