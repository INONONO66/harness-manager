use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

pub fn demo_manifest(id: &str) -> String {
    format!(
        r#"schema_version = 1
id = "{id}"
display_name = "Demo Harness"
target_runtime = "Codex CLI"
detect_binaries = ["demo"]
launch_binary = "demo"

[package]
kind = "npm-global"
package = "demo-package"

[isolation]
spoof_home = true
home_subdirs = []
static_envs = {{ CODEX_HOME = "{{home}}/.codex" }}
"#
    )
}

pub fn git_repo_with_manifest(root: &Path, manifest: &str) {
    fs::write(root.join("harness.toml"), manifest).unwrap();
    Command::new("git")
        .args(["init"])
        .current_dir(root)
        .status()
        .unwrap();
    Command::new("git")
        .args(["add", "harness.toml"])
        .current_dir(root)
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
        .current_dir(root)
        .status()
        .unwrap();
}

pub fn fake_npm(bin_dir: &Path, log_path: &Path) {
    let npm = bin_dir.join("npm");
    fs::write(
        &npm,
        format!(
            "#!/bin/sh\nif [ -e \"$HOME/.codex/auth.json\" ]; then printf 'auth_link_visible\\n' >> '{}'; fi\nprintf '%s\\n' \"$@\" >> '{}'\nexit 0\n",
            log_path.display(),
            log_path.display()
        ),
    )
    .unwrap();
    let mut perms = fs::metadata(&npm).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(npm, perms).unwrap();
}
