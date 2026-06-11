use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

#[test]
fn init_install_exits_nonzero_when_harness_install_fails() {
    // Given: every package manager shim fails during init --install.
    let temp = tempfile::tempdir().unwrap();
    let bin = temp.path().join("bin");
    let data = temp.path().join("data");
    let config = temp.path().join("config");
    let home = temp.path().join("home");
    fs::create_dir_all(&bin).unwrap();
    fs::create_dir_all(&home).unwrap();
    for name in ["npm", "npx", "bunx", "uv", "pipx", "pip", "pip3"] {
        write_failing_installer(&bin, name);
    }

    // When: the user initializes and asks hm to install built-in harnesses.
    let output = Command::new(env!("CARGO_BIN_EXE_hm"))
        .args(["init", "--install"])
        .env("XDG_DATA_HOME", &data)
        .env("XDG_CONFIG_HOME", &config)
        .env("HOME", &home)
        .env("PATH", format!("{}:/usr/bin:/bin", bin.display()))
        .output()
        .unwrap();

    // Then: hm preserves the install summary and reports failure to the caller.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success(),
        "hm init --install must exit non-zero when installs fail\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("Install Summary"),
        "summary should remain visible\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Failed"),
        "failed count should remain visible\nstdout:\n{stdout}"
    );
    assert!(
        stdout.contains("install failed for harness"),
        "per-harness install error should remain visible\nstdout:\n{stdout}"
    );
    assert!(
        stderr.contains("failed"),
        "installer stderr should remain visible\nstderr:\n{stderr}"
    );
}

fn write_failing_installer(bin: &std::path::Path, name: &str) {
    let path = bin.join(name);
    fs::write(
        &path,
        format!("#!/bin/sh\nprintf '{name} failed\\n' >&2\nexit 42\n"),
    )
    .unwrap();
    let mut perms = fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(path, perms).unwrap();
}
