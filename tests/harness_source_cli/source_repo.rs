use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use super::support::{demo_manifest, fake_npm, git_repo_with_manifest};

fn custom_manifest(id: &str) -> String {
    format!(
        r#"schema_version = 1
id = "{id}"
display_name = "Custom Harness"
target_runtime = "Codex CLI"
detect_binaries = ["custom-demo"]
launch_binary = "custom-demo"

[package]
kind = "custom"
install = ["installer", "install", "demo"]
update = ["installer", "upgrade", "demo"]
uninstall = ["installer", "remove", "demo"]
bin_subdir = ".custom/bin"
self_update = "managed-by-hm"

[isolation]
home_subdirs = []
static_envs = {{ CODEX_HOME = "{{home}}/.codex" }}
"#
    )
}

fn fake_installer(bin_dir: &Path, log_path: &Path) {
    let installer = bin_dir.join("installer");
    fs::write(
        &installer,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" >> '{}'\nexit 0\n",
            log_path.display()
        ),
    )
    .unwrap();
    let mut perms = fs::metadata(&installer).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(installer, perms).unwrap();
}

#[test]
fn harness_install_source_with_alias_registers_and_installs() {
    // Given: a git harness source and a fake npm on PATH.
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let bin = temp.path().join("bin");
    let data = temp.path().join("data");
    let config = temp.path().join("config");
    let home = temp.path().join("home");
    fs::create_dir_all(&repo).unwrap();
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(&home).unwrap();
    git_repo_with_manifest(&repo, &demo_manifest("upstream-demo"));
    let npm_log = temp.path().join("npm.log");
    fake_npm(&bin, &npm_log);

    // When: the user installs directly from the source with a chosen alias.
    let output = Command::new(env!("CARGO_BIN_EXE_hm"))
        .args([
            "harness",
            "install",
            repo.to_str().unwrap(),
            "--alias",
            "team-demo",
        ])
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CONFIG_HOME", &config)
        .env("HOME", &home)
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .output()
        .unwrap();

    // Then: hm stores the aliased manifest and runs the existing install path.
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let manifest = fs::read_to_string(data.join("hm/plugins/team-demo/harness.toml")).unwrap();
    assert!(manifest.contains("id = \"team-demo\""));
    assert_eq!(
        fs::read_to_string(npm_log).unwrap(),
        "install\n-g\ndemo-package\n"
    );
}

#[test]
fn harness_custom_source_lifecycle_uses_manifest_argv_and_records_manager() {
    // Given: a source repo declaring a custom package backend.
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let bin = temp.path().join("bin");
    let data = temp.path().join("data");
    let config = temp.path().join("config");
    let home = temp.path().join("home");
    fs::create_dir_all(&repo).unwrap();
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(&home).unwrap();
    git_repo_with_manifest(&repo, &custom_manifest("upstream-custom"));
    let installer_log = temp.path().join("installer.log");
    fake_installer(&bin, &installer_log);

    let env_path = format!("{}:/usr/bin:/bin", bin.display());

    // When: the user installs, updates, and removes the aliased custom harness.
    let install = Command::new(env!("CARGO_BIN_EXE_hm"))
        .args([
            "harness",
            "install",
            repo.to_str().unwrap(),
            "--alias",
            "custom-demo",
        ])
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CONFIG_HOME", &config)
        .env("HOME", &home)
        .env("PATH", &env_path)
        .output()
        .unwrap();
    let update = Command::new(env!("CARGO_BIN_EXE_hm"))
        .args(["harness", "update", "custom-demo"])
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CONFIG_HOME", &config)
        .env("HOME", &home)
        .env("PATH", &env_path)
        .output()
        .unwrap();
    let remove = Command::new(env!("CARGO_BIN_EXE_hm"))
        .args(["harness", "remove", "custom-demo"])
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CONFIG_HOME", &config)
        .env("HOME", &home)
        .env("PATH", &env_path)
        .output()
        .unwrap();

    // Then: hm runs the manifest argv directly and clears install state after removal.
    assert!(
        install.status.success(),
        "install stderr:\n{}",
        String::from_utf8_lossy(&install.stderr)
    );
    assert!(
        update.status.success(),
        "update stderr:\n{}",
        String::from_utf8_lossy(&update.stderr)
    );
    assert!(
        remove.status.success(),
        "remove stderr:\n{}",
        String::from_utf8_lossy(&remove.stderr)
    );
    assert_eq!(
        fs::read_to_string(installer_log).unwrap(),
        "install\ndemo\nupgrade\ndemo\nremove\ndemo\n"
    );
    assert!(
        !data
            .join("hm/runtimes/custom-demo/state/package-manager")
            .exists(),
        "remove must clear package-manager state so wrapper harnesses stop reporting Installed"
    );
}

#[test]
fn harness_add_source_rejects_duplicate_alias_without_replacing_existing_manifest() {
    // Given: an existing plugin alias and another source wanting the same alias.
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let data = temp.path().join("data");
    let config = temp.path().join("config");
    let home = temp.path().join("home");
    let existing = data.join("hm/plugins/team-demo");
    fs::create_dir_all(&repo).unwrap();
    fs::create_dir_all(&existing).unwrap();
    fs::create_dir_all(&home).unwrap();
    git_repo_with_manifest(&repo, &demo_manifest("upstream-demo"));
    fs::write(existing.join("harness.toml"), demo_manifest("team-demo")).unwrap();

    // When: the user adds the source with the duplicate alias.
    let output = Command::new(env!("CARGO_BIN_EXE_hm"))
        .args([
            "harness",
            "add",
            repo.to_str().unwrap(),
            "--alias",
            "team-demo",
        ])
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CONFIG_HOME", &config)
        .env("HOME", &home)
        .output()
        .unwrap();

    // Then: hm fails clearly and leaves the existing manifest intact.
    assert!(!output.status.success(), "duplicate alias should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("already registered"), "stderr:\n{stderr}");
    assert_eq!(
        fs::read_to_string(existing.join("harness.toml")).unwrap(),
        demo_manifest("team-demo")
    );
}

#[test]
fn harness_add_source_rejects_alias_registered_in_config_without_promoting_plugin() {
    // Given: an alias already registered from the config discovery path.
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let data = temp.path().join("data");
    let config = temp.path().join("config");
    let home = temp.path().join("home");
    let config_harnesses = config.join("hm/harnesses.d");
    fs::create_dir_all(&repo).unwrap();
    fs::create_dir_all(&config_harnesses).unwrap();
    fs::create_dir_all(&home).unwrap();
    git_repo_with_manifest(&repo, &demo_manifest("upstream-demo"));
    fs::write(
        config_harnesses.join("team-demo.toml"),
        demo_manifest("team-demo"),
    )
    .unwrap();

    // When: the user adds a source with the same alias.
    let output = Command::new(env!("CARGO_BIN_EXE_hm"))
        .args([
            "harness",
            "add",
            repo.to_str().unwrap(),
            "--alias",
            "team-demo",
        ])
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CONFIG_HOME", &config)
        .env("HOME", &home)
        .output()
        .unwrap();

    // Then: hm rejects the duplicate before promoting the plugin.
    assert!(!output.status.success(), "duplicate alias should fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("already registered"), "stderr:\n{stderr}");
    assert!(
        !data.join("hm/plugins/team-demo").exists(),
        "duplicate source must not be promoted"
    );
    let list = Command::new(env!("CARGO_BIN_EXE_hm"))
        .args(["harness", "list"])
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CONFIG_HOME", &config)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(
        list.status.success(),
        "registry should remain usable:\n{}",
        String::from_utf8_lossy(&list.stderr)
    );
}

#[test]
fn harness_install_source_rejects_invalid_manifest_without_poisoning_registry() {
    // Given: a source repo whose manifest becomes invalid after aliasing.
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let data = temp.path().join("data");
    let config = temp.path().join("config");
    let home = temp.path().join("home");
    fs::create_dir_all(&repo).unwrap();
    fs::create_dir_all(&home).unwrap();
    git_repo_with_manifest(
        &repo,
        &demo_manifest("upstream-demo").replace("Codex CLI", "missing-runtime"),
    );

    // When: the user tries to install from that source.
    let output = Command::new(env!("CARGO_BIN_EXE_hm"))
        .args([
            "harness",
            "install",
            repo.to_str().unwrap(),
            "--alias",
            "bad-demo",
        ])
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CONFIG_HOME", &config)
        .env("HOME", &home)
        .output()
        .unwrap();

    // Then: hm fails before leaving a broken plugin in discovery.
    assert!(!output.status.success(), "invalid manifest should fail");
    assert!(
        !data.join("hm/plugins/bad-demo").exists(),
        "invalid source must not be promoted"
    );
    let list = Command::new(env!("CARGO_BIN_EXE_hm"))
        .args(["harness", "list"])
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CONFIG_HOME", &config)
        .env("HOME", &home)
        .output()
        .unwrap();
    assert!(
        list.status.success(),
        "registry should not be poisoned:\n{}",
        String::from_utf8_lossy(&list.stderr)
    );
}

#[test]
fn harness_add_source_then_install_alias_works() {
    // Given: a git harness source and a fake npm on PATH.
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().join("repo");
    let bin = temp.path().join("bin");
    let data = temp.path().join("data");
    let config = temp.path().join("config");
    let home = temp.path().join("home");
    fs::create_dir_all(&repo).unwrap();
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(&home).unwrap();
    git_repo_with_manifest(&repo, &demo_manifest("upstream-demo"));
    let npm_log = temp.path().join("npm.log");
    fake_npm(&bin, &npm_log);

    // When: the user adds the source and later installs by alias.
    let add = Command::new(env!("CARGO_BIN_EXE_hm"))
        .args([
            "harness",
            "add",
            repo.to_str().unwrap(),
            "--alias",
            "team-demo",
        ])
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CONFIG_HOME", &config)
        .env("HOME", &home)
        .output()
        .unwrap();
    let install = Command::new(env!("CARGO_BIN_EXE_hm"))
        .args(["harness", "install", "team-demo"])
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CONFIG_HOME", &config)
        .env("HOME", &home)
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .output()
        .unwrap();

    // Then: the registered alias installs through the existing install path.
    assert!(
        add.status.success(),
        "add stderr:\n{}",
        String::from_utf8_lossy(&add.stderr)
    );
    assert!(
        install.status.success(),
        "install stderr:\n{}",
        String::from_utf8_lossy(&install.stderr)
    );
    assert_eq!(
        fs::read_to_string(npm_log).unwrap(),
        "install\n-g\ndemo-package\n"
    );
}
