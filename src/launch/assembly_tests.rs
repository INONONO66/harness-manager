use std::collections::HashMap;

use tempfile::tempdir;

use super::*;

#[test]
fn python_tool_binary_override_adds_isolated_local_bin() {
    let tmp = tempdir().expect("tempdir");
    let home = tmp.path().join("home");
    let bin_dir = home.join(".local").join("bin");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    let binary = bin_dir.join("python-tool-bin");
    std::fs::write(&binary, "#!/bin/sh\n").expect("write binary");

    let iso_paths = Some(isolation::IsolationPaths {
        base: tmp.path().join("base"),
        home,
        state: tmp.path().join("state"),
        tmp: tmp.path().join("tmp"),
        runtime_base: tmp.path().join("runtime-base"),
        runtime_home: tmp.path().join("runtime-home"),
        runtime_state: tmp.path().join("runtime-state"),
        runtime_logs: tmp.path().join("runtime-logs"),
    });
    let mut env = HashMap::from([("PATH".to_string(), "/usr/bin".to_string())]);

    let override_path = isolated_package_binary_override(
        Some(".local/bin"),
        &iso_paths,
        &mut env,
        &["python-tool-bin".to_string()],
    );

    assert_eq!(override_path, Some(binary));
    assert_eq!(
        env.get("PATH").expect("PATH"),
        &format!("{}:/usr/bin", bin_dir.to_string_lossy())
    );
}

#[test]
fn declared_bin_subdir_override_adds_custom_bin_dir() {
    let tmp = tempdir().expect("tempdir");
    let home = tmp.path().join("home");
    let bin_dir = home.join(".custom").join("bin");
    std::fs::create_dir_all(&bin_dir).expect("create bin dir");
    let binary = bin_dir.join("custom-tool");
    std::fs::write(&binary, "#!/bin/sh\n").expect("write binary");

    let iso_paths = Some(isolation::IsolationPaths {
        base: tmp.path().join("base"),
        home,
        state: tmp.path().join("state"),
        tmp: tmp.path().join("tmp"),
        runtime_base: tmp.path().join("runtime-base"),
        runtime_home: tmp.path().join("runtime-home"),
        runtime_state: tmp.path().join("runtime-state"),
        runtime_logs: tmp.path().join("runtime-logs"),
    });
    let mut env = HashMap::from([("PATH".to_string(), "/usr/bin".to_string())]);

    let override_path = isolated_package_binary_override(
        Some(".custom/bin"),
        &iso_paths,
        &mut env,
        &["custom-tool".to_string()],
    );

    assert_eq!(override_path, Some(binary));
    assert_eq!(
        env.get("PATH").expect("PATH"),
        &format!("{}:/usr/bin", bin_dir.to_string_lossy())
    );
}

fn test_isolation(subdir: &str) -> isolation::spec::IsolationPlan {
    isolation::spec::IsolationPlan {
        subdir: subdir.to_string(),
        runtime_subdir: subdir.to_string(),
        spoof_home: false,
        home_subdirs: Vec::new(),
        static_envs: Vec::new(),
        seed_files: Vec::new(),
        caveat: None,
    }
}

fn custom_wrapper_harness(subdir: &str) -> crate::harnesses::manifest::ManifestHarnessSpec {
    crate::harnesses::manifest::ManifestHarnessSpec {
        id: subdir.to_string(),
        aliases: Vec::new(),
        display_name: subdir.to_string(),
        target_runtime: "shell".to_string(),
        target_runtime_shared_state: None,
        package: crate::harnesses::manifest::ManifestPackageSpec::Custom {
            install: crate::harnesses::manifest::PackageCommandTemplate {
                argv: vec!["installer".to_string(), "install".to_string()],
            },
            update: None,
            uninstall: None,
            bin_subdir: None,
            self_update: None,
        },
        detect_binaries: vec!["sh".to_string()],
        launch_binary: Some("sh".to_string()),
        launch_args: Vec::new(),
        isolation: test_isolation(subdir),
    }
}

#[test]
fn package_managed_wrapper_launch_requires_package_state() {
    let subdir = format!("hm-launch-wrapper-no-state-{}", std::process::id());
    let harness = custom_wrapper_harness(&subdir);

    let result = ensure_package_managed_harness_installed(&harness);

    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("hm harness install"),
        "missing package state should guide the user to install the harness"
    );

    let paths = isolation::IsolationPaths::try_from_spec(&harness.isolation).unwrap();
    let _ = std::fs::remove_dir_all(&paths.base);
}

#[test]
fn package_managed_wrapper_launch_allows_recorded_package_state() {
    let subdir = format!("hm-launch-wrapper-state-{}", std::process::id());
    let harness = custom_wrapper_harness(&subdir);
    let paths = isolation::IsolationPaths::try_from_spec(&harness.isolation).unwrap();
    crate::harnesses::state::record_package_manager(&paths, "installer").unwrap();

    ensure_package_managed_harness_installed(&harness).unwrap();

    let _ = std::fs::remove_dir_all(&paths.base);
}
