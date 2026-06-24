use super::*;

use std::fs;
use std::process::Command;

use tempfile::TempDir;

use crate::runtimes::registry::RuntimeRegistry;

fn demo_manifest(id: &str) -> String {
    format!(
        r#"schema_version = 1
id = "{id}"
display_name = "Demo Harness"
target_runtime = "Codex CLI"
detect_binaries = ["demo"]
launch_binary = "demo"

[package]
kind = "manual"
instructions = "demo only"

[isolation]
home_subdirs = []
static_envs = {{ CODEX_HOME = "{{home}}/.codex" }}
"#
    )
}

fn git_repo_with_manifest(manifest: &str) -> TempDir {
    let repo = tempfile::tempdir().unwrap();
    fs::write(repo.path().join("harness.toml"), manifest).unwrap();
    Command::new("git")
        .args(["init"])
        .current_dir(repo.path())
        .status()
        .unwrap();
    Command::new("git")
        .args(["add", "harness.toml"])
        .current_dir(repo.path())
        .status()
        .unwrap();
    Command::new("git")
        .args([
            "-c",
            "user.email=test@example.com",
            "-c",
            "user.name=Test",
            "commit",
            "-m",
            "init",
        ])
        .current_dir(repo.path())
        .status()
        .unwrap();
    repo
}

fn test_runtimes() -> RuntimeRegistry {
    RuntimeRegistry::builtin_only().unwrap()
}

#[test]
fn add_source_clones_manifest_and_rewrites_id_to_alias() {
    // Given: a git repository containing a reusable harness manifest.
    let repo = git_repo_with_manifest(&demo_manifest("upstream-demo"));
    let data_home = tempfile::tempdir().unwrap();
    let runtimes = test_runtimes();

    // When: the source is added under a user-chosen alias.
    let installed = add_harness_source(
        repo.path().to_str().unwrap(),
        "team-demo",
        data_home.path(),
        &runtimes,
    )
    .expect("source installs");

    // Then: hm discovers a plugin-shaped manifest keyed by that alias.
    assert_eq!(installed.alias(), "team-demo");
    let manifest_path = data_home
        .path()
        .join("hm")
        .join("plugins")
        .join("team-demo")
        .join("harness.toml");
    let manifest = fs::read_to_string(manifest_path).unwrap();
    assert!(
        manifest.contains("id = \"team-demo\""),
        "manifest should be aliased for command discovery:\n{manifest}"
    );
    assert!(
        !manifest.contains("upstream-demo"),
        "source id must not leak into the installed command alias:\n{manifest}"
    );
}

#[test]
fn add_source_rejects_duplicate_alias_without_partial_state() {
    // Given: an alias directory that is already registered.
    let repo = git_repo_with_manifest(&demo_manifest("upstream-demo"));
    let data_home = tempfile::tempdir().unwrap();
    let runtimes = test_runtimes();
    let existing = data_home
        .path()
        .join("hm")
        .join("plugins")
        .join("team-demo");
    fs::create_dir_all(&existing).unwrap();
    fs::write(existing.join("harness.toml"), demo_manifest("team-demo")).unwrap();

    // When: another source tries to use the same alias.
    let err = add_harness_source(
        repo.path().to_str().unwrap(),
        "team-demo",
        data_home.path(),
        &runtimes,
    )
    .unwrap_err();

    // Then: the existing plugin is preserved and no temp install is promoted.
    assert!(
        err.to_string().contains("already registered"),
        "unexpected error: {err:#}"
    );
    assert_eq!(
        fs::read_to_string(existing.join("harness.toml")).unwrap(),
        demo_manifest("team-demo")
    );
}

#[test]
fn add_source_rejects_missing_harness_manifest() {
    // Given: a git repository without harness.toml.
    let repo = git_repo_with_manifest(&demo_manifest("upstream-demo"));
    let runtimes = test_runtimes();
    fs::remove_file(repo.path().join("harness.toml")).unwrap();
    Command::new("git")
        .args(["add", "-A"])
        .current_dir(repo.path())
        .status()
        .unwrap();
    Command::new("git")
        .args([
            "-c",
            "user.email=test@example.com",
            "-c",
            "user.name=Test",
            "commit",
            "-m",
            "remove manifest",
        ])
        .current_dir(repo.path())
        .status()
        .unwrap();
    let data_home = tempfile::tempdir().unwrap();

    // When: the source is added.
    let err = add_harness_source(
        repo.path().to_str().unwrap(),
        "team-demo",
        data_home.path(),
        &runtimes,
    )
    .unwrap_err();

    // Then: hm reports the missing contract and leaves no plugin directory.
    assert!(
        err.to_string().contains("harness.toml"),
        "unexpected error: {err:#}"
    );
    assert!(!data_home
        .path()
        .join("hm")
        .join("plugins")
        .join("team-demo")
        .exists());
}
