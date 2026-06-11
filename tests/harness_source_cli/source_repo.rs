use std::fs;
use std::process::Command;

use super::support::{demo_manifest, fake_npm, git_repo_with_manifest};

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
