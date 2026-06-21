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
