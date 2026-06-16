use super::{minimal_manifest, parse_toml};
use crate::harnesses::manifest::ManifestPackageSpec;

#[test]
fn parses_minimal_manifest_into_owned_spec() {
    // Given: a minimal valid manifest.
    let input = minimal_manifest("");

    // When: the manifest is parsed.
    let parsed = parse_toml("demo.toml", &input).expect("manifest parses");

    // Then: owned fields and isolation defaults are preserved.
    assert_eq!(parsed.id, "demo");
    assert!(parsed.aliases.is_empty());
    assert_eq!(parsed.display_name, "Demo Harness");
    assert_eq!(parsed.target_runtime, "Codex CLI");
    assert_eq!(parsed.detect_binaries, vec!["demo"]);
    assert_eq!(parsed.launch_binary, None);
    assert!(parsed.launch_args.is_empty());
    assert_eq!(parsed.isolation.subdir, "demo");
    assert_eq!(parsed.isolation.runtime_subdir, "demo");
    let shared_state = parsed
        .target_runtime_shared_state
        .expect("target runtime shared-state policy");
    assert_eq!(shared_state.database_dirs, vec![".codex"]);
    assert_eq!(shared_state.auth_files, vec![".codex/auth.json"]);
    assert_eq!(parsed.isolation.home_subdirs, vec![".codex"]);
    assert_eq!(
        parsed.isolation.static_envs,
        vec![("CODEX_HOME".to_string(), "{home}/.codex".to_string())]
    );
    assert!(parsed.isolation.seed_files.is_empty());
    match parsed.package {
        ManifestPackageSpec::NpmGlobal { package, .. } => assert_eq!(package, "demo-package"),
        other => panic!("unexpected package variant: {other:?}"),
    }
}

#[test]
fn manifest_parses_aliases() {
    // Given: a manifest that declares short command aliases.
    let input = minimal_manifest("").replace(
        r#"id = "demo""#,
        "id = \"demo\"\naliases = [\"dm\", \"dx\"]",
    );

    // When: the manifest is parsed.
    let parsed = parse_toml("aliases.toml", &input).expect("aliases parse");

    // Then: aliases are preserved in declaration order.
    assert_eq!(parsed.aliases, vec!["dm", "dx"]);
}

#[test]
fn manifest_parses_default_launch_args() {
    // Given: one manifest with omitted launch args and one with fixed launch args.
    let omitted = minimal_manifest("");
    let with_args = minimal_manifest("").replace(
        r#"detect_binaries = ["demo"]"#,
        "detect_binaries = [\"demo\"]\nlaunch_args = [\"run\", \"--fast\"]",
    );

    // When: both manifests are parsed.
    let defaulted = parse_toml("default.toml", &omitted).expect("default parses");
    let explicit = parse_toml("explicit.toml", &with_args).expect("explicit parses");

    // Then: omitted launch args default to empty and explicit args keep order.
    assert!(defaulted.launch_args.is_empty());
    assert_eq!(explicit.launch_args, vec!["run", "--fast"]);
}

#[test]
fn manifest_parses_custom_package_backend_with_bin_subdir() {
    // Given: a manifest-owned backend described as argv arrays, not shell.
    let input = minimal_manifest("").replace(
        r#"[package]
kind = "npm-global"
package = "demo-package"
"#,
        r#"[package]
kind = "custom"
install = ["installer", "install", "demo"]
update = ["installer", "upgrade", "demo"]
uninstall = ["installer", "remove", "demo"]
bin_subdir = ".custom/bin"
self_update = "managed-by-hm"
"#,
    );

    // When: the manifest is parsed.
    let parsed = parse_toml("custom.toml", &input).expect("custom backend parses");

    // Then: the backend stays declarative and carries its bin layout.
    match parsed.package {
        ManifestPackageSpec::Custom {
            install,
            update,
            uninstall,
            bin_subdir,
            ..
        } => {
            assert_eq!(install.argv, vec!["installer", "install", "demo"]);
            assert_eq!(update.unwrap().argv, vec!["installer", "upgrade", "demo"]);
            assert_eq!(uninstall.unwrap().argv, vec!["installer", "remove", "demo"]);
            assert_eq!(bin_subdir.as_deref(), Some(".custom/bin"));
        }
        other => panic!("unexpected package variant: {other:?}"),
    }
}

#[test]
fn manifest_parses_git_worktree_package_backend() {
    // Given: a repo-backed package that runs a checked-in setup program.
    let input = minimal_manifest("").replace(
        r#"[package]
kind = "npm-global"
package = "demo-package"
"#,
        r#"[package]
kind = "git-worktree"
repository = "https://github.com/example/demo"
setup = ["setup", "--host", "codex"]
update = ["setup", "--host", "codex", "--update"]
self_update = "managed-by-hm"
"#,
    );

    // When: the manifest is parsed.
    let parsed = parse_toml("git-worktree.toml", &input).expect("git worktree backend parses");

    // Then: the repo URL and setup commands stay structured.
    match parsed.package {
        ManifestPackageSpec::GitWorktree {
            repository,
            setup,
            update,
            ..
        } => {
            assert_eq!(repository, "https://github.com/example/demo");
            assert_eq!(setup.argv, vec!["setup", "--host", "codex"]);
            assert_eq!(
                update.expect("update command").argv,
                vec!["setup", "--host", "codex", "--update"]
            );
        }
        other => panic!("unexpected package variant: {other:?}"),
    }
}
