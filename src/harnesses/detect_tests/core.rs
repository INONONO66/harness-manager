use super::*;

#[test]
fn detect_one_finds_sh() {
    let spec = HarnessSpec {
        id: "test-sh".to_string(),
        aliases: Vec::new(),
        display_name: "test".to_string(),
        target_runtime: "Codex CLI".to_string(),
        target_runtime_shared_state: None,
        package: PackageSpec::Manual {
            instructions: "".to_string(),
            self_update: None,
        },
        detect_binaries: vec!["sh".to_string()],
        isolation: empty_iso("test"),
        launch_binary: None,
        launch_args: Vec::new(),
    };
    let result = detect_one(&spec);
    assert!(result.installed, "sh should be found on PATH");
    assert!(result.binary_path.is_some());
}

#[test]
fn detect_one_missing_binary() {
    let spec = HarnessSpec {
        id: "test-missing".to_string(),
        aliases: Vec::new(),
        display_name: "test".to_string(),
        target_runtime: "Codex CLI".to_string(),
        target_runtime_shared_state: None,
        package: PackageSpec::Manual {
            instructions: "".to_string(),
            self_update: None,
        },
        detect_binaries: vec!["nonexistent-binary-xyz-99".to_string()],
        isolation: empty_iso("test"),
        launch_binary: None,
        launch_args: Vec::new(),
    };
    let result = detect_one(&spec);
    assert!(!result.installed);
    assert!(result.binary_path.is_none());
}

#[test]
fn detect_all_returns_registered_harnesses() {
    let runtimes = test_runtimes();
    let registry = crate::harnesses::registry::HarnessRegistry::from_sources(
        &[crate::harnesses::registry::HarnessSource::manifest(
            "detect-plugin.toml",
            r#"
schema_version = 1
id = "detect-plugin"
display_name = "Detect Plugin"
target_runtime = "Codex CLI"
detect_binaries = ["nonexistent-detect-plugin-bin"]
launch_args = []

[package]
kind = "manual"
instructions = "manual"

[isolation]
home_subdirs = [".codex"]
static_envs = { CODEX_HOME = "{home}/.codex" }
seed_files = []
"#,
        )],
        &runtimes,
    )
    .unwrap();

    let results = detect_all(&registry);

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "detect-plugin");
    assert_eq!(results[0].target_runtime, "Codex CLI");
    assert!(!results[0].installed);
}

#[test]
fn detect_all_builtin_only_returns_indexed_builtins() {
    let registry =
        crate::harnesses::registry::HarnessRegistry::builtin_only(&test_runtimes()).unwrap();
    let results = detect_all(&registry);

    assert_eq!(
        results.len(),
        crate::harnesses::builtin::BUILTIN_MANIFESTS.len()
    );
}

#[test]
fn format_package_source_covers_every_kind() {
    assert_eq!(
        format_package_source(&PackageSpec::NpmGlobal {
            package: "p".to_string(),
            self_update: None,
        }),
        "npm-global (p)"
    );
    assert_eq!(
        format_package_source(&PackageSpec::NpmIsolated {
            package: "p".to_string(),
            self_update: None,
        }),
        "npm-isolated (p)"
    );
    assert_eq!(
        format_package_source(&PackageSpec::NpxInstaller {
            package: "p".to_string(),
            args: Vec::new(),
            self_update: None,
        }),
        "npx-installer (p)"
    );
    assert_eq!(
        format_package_source(&PackageSpec::BunxInstaller {
            package: "p".to_string(),
            args: Vec::new(),
            self_update: None,
        }),
        "bunx-installer (p)"
    );
    assert_eq!(
        format_package_source(&PackageSpec::PythonTool {
            package: "p".to_string(),
            self_update: None,
        }),
        "python-tool (p)"
    );
    assert_eq!(
        format_package_source(&PackageSpec::Manual {
            instructions: "x".to_string(),
            self_update: None,
        }),
        "manual"
    );
    assert_eq!(
        format_package_source(&PackageSpec::GitWorktree {
            repository: "https://github.com/example/demo".to_string(),
            setup: PackageCommandTemplate {
                argv: vec!["setup".to_string()],
            },
            update: None,
            self_update: None,
        }),
        "git-worktree (https://github.com/example/demo)"
    );
}
