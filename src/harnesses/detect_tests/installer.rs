use super::*;

#[test]
fn detect_one_marks_npx_installer_as_wrapping_runtime_binary() {
    let subdir = unique_subdir("wrapping-runtime");
    let isolation = empty_iso(&subdir);
    let paths = crate::isolation::IsolationPaths::try_from_spec(&isolation).unwrap();
    let cache_pkg = paths
        .home
        .join(".npm")
        .join("_npx")
        .join("seedhash")
        .join("node_modules")
        .join("lazysh-ai");
    std::fs::create_dir_all(&cache_pkg).expect("seed npx cache");

    let spec = HarnessSpec {
        id: "lazysh".to_string(),
        aliases: Vec::new(),
        display_name: "lazysh".to_string(),
        target_runtime: "shell".to_string(),
        target_runtime_shared_state: None,
        package: PackageSpec::NpxInstaller {
            package: "lazysh-ai".to_string(),
            args: Vec::new(),
            self_update: None,
        },
        detect_binaries: vec!["sh".to_string()],
        isolation,
        launch_binary: Some("sh".to_string()),
        launch_args: Vec::new(),
    };
    let result = detect_one(&spec);

    assert!(
        result.installed,
        "NpxInstaller with cached package must report installed"
    );
    assert!(result.wraps_target_runtime_binary);
    assert_eq!(result.package_source, "npx-installer (lazysh-ai)");

    let _ = std::fs::remove_dir_all(&paths.base);
}

#[test]
fn detect_one_npx_installer_not_installed_when_cache_empty_even_with_runtime_on_path() {
    // Regression for issue #5: PATH-only lookup falsely reported Installed
    // for NpxInstaller harnesses when only the target runtime binary was
    // on host PATH after `hm harness remove --purge`.
    let subdir = unique_subdir("npx-no-cache");
    let isolation = empty_iso(&subdir);

    let spec = HarnessSpec {
        id: "lazysh".to_string(),
        aliases: Vec::new(),
        display_name: "lazysh".to_string(),
        target_runtime: "shell".to_string(),
        target_runtime_shared_state: None,
        package: PackageSpec::NpxInstaller {
            package: "lazysh-ai".to_string(),
            args: Vec::new(),
            self_update: None,
        },
        detect_binaries: vec!["sh".to_string()],
        isolation: isolation.clone(),
        launch_binary: Some("sh".to_string()),
        launch_args: Vec::new(),
    };
    let result = detect_one(&spec);

    assert!(
        !result.installed,
        "NpxInstaller without npx cache must NOT report installed even when runtime binary is on PATH"
    );
    assert!(
        result.binary_path.is_none(),
        "NpxInstaller without npx cache must report no binary path"
    );

    let paths = crate::isolation::IsolationPaths::try_from_spec(&isolation).unwrap();
    let _ = std::fs::remove_dir_all(&paths.base);
}

#[test]
fn detect_one_bunx_installer_not_installed_when_cache_empty_even_with_runtime_on_path() {
    // Regression for issue #5 (BunxInstaller variant).
    let subdir = unique_subdir("bunx-no-cache");
    let isolation = empty_iso(&subdir);

    let spec = HarnessSpec {
        id: "bunsh".to_string(),
        aliases: Vec::new(),
        display_name: "bunsh".to_string(),
        target_runtime: "shell".to_string(),
        target_runtime_shared_state: None,
        package: PackageSpec::BunxInstaller {
            package: "oh-my-bunsh".to_string(),
            args: Vec::new(),
            self_update: None,
        },
        detect_binaries: vec!["sh".to_string()],
        isolation: isolation.clone(),
        launch_binary: Some("sh".to_string()),
        launch_args: Vec::new(),
    };
    let result = detect_one(&spec);

    assert!(
        !result.installed,
        "BunxInstaller without bun cache must NOT report installed even when runtime binary is on PATH"
    );
    assert!(result.binary_path.is_none());

    let paths = crate::isolation::IsolationPaths::try_from_spec(&isolation).unwrap();
    let _ = std::fs::remove_dir_all(&paths.base);
}

#[test]
fn detect_one_bunx_installer_installed_when_cache_present() {
    let subdir = unique_subdir("bunx-cache");
    let isolation = empty_iso(&subdir);
    let paths = crate::isolation::IsolationPaths::try_from_spec(&isolation).unwrap();
    let cache_root = paths.home.join(".bun").join("install").join("cache");
    let pkg_versioned = cache_root.join("oh-my-bunsh@1.2.3");
    std::fs::create_dir_all(&pkg_versioned).expect("seed bunx cache");

    let spec = HarnessSpec {
        id: "bunsh".to_string(),
        aliases: Vec::new(),
        display_name: "bunsh".to_string(),
        target_runtime: "shell".to_string(),
        target_runtime_shared_state: None,
        package: PackageSpec::BunxInstaller {
            package: "oh-my-bunsh".to_string(),
            args: Vec::new(),
            self_update: None,
        },
        detect_binaries: vec!["sh".to_string()],
        isolation,
        launch_binary: Some("sh".to_string()),
        launch_args: Vec::new(),
    };
    let result = detect_one(&spec);

    assert!(
        result.installed,
        "BunxInstaller with versioned cache dir must report installed"
    );
    assert_eq!(result.package_source, "bunx-installer (oh-my-bunsh)");

    let _ = std::fs::remove_dir_all(&paths.base);
}

#[test]
fn detect_one_python_tool_installed_from_isolated_local_bin() {
    let subdir = unique_subdir("python-tool-bin");
    let isolation = empty_iso(&subdir);
    let paths = crate::isolation::IsolationPaths::try_from_spec(&isolation).unwrap();
    let bin_dir = paths.home.join(".local").join("bin");
    let binary = bin_dir.join("python-tool-bin");
    std::fs::create_dir_all(&bin_dir).expect("seed python tool bin dir");
    std::fs::write(&binary, "#!/bin/sh\n").expect("seed python tool binary");

    let spec = HarnessSpec {
        id: "python-tool-harness".to_string(),
        aliases: Vec::new(),
        display_name: "python-tool-harness".to_string(),
        target_runtime: "shell".to_string(),
        target_runtime_shared_state: None,
        package: PackageSpec::PythonTool {
            package: "python-tool-package".to_string(),
            self_update: None,
        },
        detect_binaries: vec!["python-tool-bin".to_string()],
        isolation,
        launch_binary: Some("python-tool-bin".to_string()),
        launch_args: Vec::new(),
    };
    let result = detect_one(&spec);

    assert!(
        result.installed,
        "PythonTool must report installed when pipx/uv created a binary under isolated HOME/.local/bin"
    );
    assert_eq!(result.binary_path, Some(binary));

    let _ = std::fs::remove_dir_all(&paths.base);
}

#[test]
fn detect_one_custom_backend_uses_declared_bin_subdir() {
    let subdir = unique_subdir("custom-bin");
    let isolation = empty_iso(&subdir);
    let paths = crate::isolation::IsolationPaths::try_from_spec(&isolation).unwrap();
    let bin_dir = paths.home.join(".custom").join("bin");
    let binary = bin_dir.join("custom-tool");
    std::fs::create_dir_all(&bin_dir).expect("seed custom bin dir");
    std::fs::write(&binary, "#!/bin/sh\n").expect("seed custom binary");
    crate::harnesses::state::record_package_manager(&paths, "installer").unwrap();

    let spec = HarnessSpec {
        id: "custom-tool".to_string(),
        aliases: Vec::new(),
        display_name: "custom-tool".to_string(),
        target_runtime: "shell".to_string(),
        target_runtime_shared_state: None,
        package: PackageSpec::Custom {
            install: PackageCommandTemplate {
                argv: vec!["installer".to_string(), "install".to_string()],
            },
            update: None,
            uninstall: None,
            bin_subdir: Some(".custom/bin".to_string()),
            self_update: None,
        },
        detect_binaries: vec!["custom-tool".to_string()],
        isolation,
        launch_binary: Some("custom-tool".to_string()),
        launch_args: Vec::new(),
    };
    let result = detect_one(&spec);

    assert!(result.installed);
    assert_eq!(result.binary_path, Some(binary));

    let _ = std::fs::remove_dir_all(&paths.base);
}

#[test]
fn detect_one_custom_wrapper_is_not_installed_from_runtime_binary_alone() {
    let subdir = unique_subdir("custom-wrapper-no-state");
    let isolation = empty_iso(&subdir);

    let spec = HarnessSpec {
        id: "custom-wrapper".to_string(),
        aliases: Vec::new(),
        display_name: "custom-wrapper".to_string(),
        target_runtime: "shell".to_string(),
        target_runtime_shared_state: None,
        package: PackageSpec::Custom {
            install: PackageCommandTemplate {
                argv: vec!["installer".to_string(), "install".to_string()],
            },
            update: None,
            uninstall: None,
            bin_subdir: None,
            self_update: None,
        },
        detect_binaries: vec!["sh".to_string()],
        isolation: isolation.clone(),
        launch_binary: Some("sh".to_string()),
        launch_args: Vec::new(),
    };
    let result = detect_one(&spec);

    assert!(
        !result.installed,
        "Custom wrapper without package state must NOT report installed from a target runtime binary on PATH"
    );
    assert!(result.binary_path.is_none());

    let paths = crate::isolation::IsolationPaths::try_from_spec(&isolation).unwrap();
    let _ = std::fs::remove_dir_all(&paths.base);
}

#[test]
fn detect_one_custom_wrapper_installed_when_package_state_exists() {
    let subdir = unique_subdir("custom-wrapper-state");
    let isolation = empty_iso(&subdir);
    let paths = crate::isolation::IsolationPaths::try_from_spec(&isolation).unwrap();
    crate::harnesses::state::record_package_manager(&paths, "installer").unwrap();

    let spec = HarnessSpec {
        id: "custom-wrapper".to_string(),
        aliases: Vec::new(),
        display_name: "custom-wrapper".to_string(),
        target_runtime: "shell".to_string(),
        target_runtime_shared_state: None,
        package: PackageSpec::Custom {
            install: PackageCommandTemplate {
                argv: vec!["installer".to_string(), "install".to_string()],
            },
            update: None,
            uninstall: None,
            bin_subdir: None,
            self_update: None,
        },
        detect_binaries: vec!["sh".to_string()],
        isolation,
        launch_binary: Some("sh".to_string()),
        launch_args: Vec::new(),
    };
    let result = detect_one(&spec);

    assert!(result.installed);
    assert!(result.binary_path.is_some());

    let _ = std::fs::remove_dir_all(&paths.base);
}

#[test]
fn detect_one_git_worktree_wrapper_is_not_installed_from_runtime_binary_alone() {
    let subdir = unique_subdir("git-wrapper-no-state");
    let isolation = empty_iso(&subdir);

    let spec = HarnessSpec {
        id: "git-wrapper".to_string(),
        aliases: Vec::new(),
        display_name: "git-wrapper".to_string(),
        target_runtime: "shell".to_string(),
        target_runtime_shared_state: None,
        package: PackageSpec::GitWorktree {
            repository: "https://github.com/example/demo".to_string(),
            setup: PackageCommandTemplate {
                argv: vec!["setup".to_string()],
            },
            update: None,
            self_update: None,
        },
        detect_binaries: vec!["sh".to_string()],
        isolation: isolation.clone(),
        launch_binary: Some("sh".to_string()),
        launch_args: Vec::new(),
    };
    let result = detect_one(&spec);

    assert!(
        !result.installed,
        "Git worktree wrapper without package state must NOT report installed from a target runtime binary on PATH"
    );
    assert!(result.binary_path.is_none());

    let paths = crate::isolation::IsolationPaths::try_from_spec(&isolation).unwrap();
    let _ = std::fs::remove_dir_all(&paths.base);
}

#[test]
fn detect_one_does_not_mark_npm_isolated_as_wrapping() {
    let spec = HarnessSpec {
        id: "iso-sh".to_string(),
        aliases: Vec::new(),
        display_name: "iso-sh".to_string(),
        target_runtime: "shell".to_string(),
        target_runtime_shared_state: None,
        package: PackageSpec::NpmIsolated {
            package: "iso-sh-pkg".to_string(),
            self_update: None,
        },
        detect_binaries: vec!["sh".to_string()],
        isolation: empty_iso("iso-sh"),
        launch_binary: None,
        launch_args: Vec::new(),
    };
    let result = detect_one(&spec);
    assert!(!result.wraps_target_runtime_binary);
    assert_eq!(result.package_source, "npm-isolated (iso-sh-pkg)");
}
