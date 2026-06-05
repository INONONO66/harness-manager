use std::fs;

use crate::isolation::{ensure_isolation_tree, seed_files};
use crate::runtimes::types::{IsolationSpec, SeedFile};

use super::tmp_paths;

#[test]
fn ensure_tree_creates_home_subdirs() {
    let p = tmp_paths("ensure-tree");
    let _ = fs::remove_dir_all(&p.base);
    let spec = IsolationSpec {
        subdir: "test",
        spoof_home: true,
        home_subdirs: &[".codex", ".config/opencode"],
        static_envs: &[],
        seed_files: &[],
        caveat: None,
    };

    ensure_isolation_tree(&spec, &p).unwrap();

    assert!(p.home.join(".codex").is_dir());
    assert!(p.home.join(".config/opencode").is_dir());
    assert!(p.state.is_dir());
    assert!(p.tmp.is_dir());
    let _ = fs::remove_dir_all(&p.base);
}

#[test]
fn seed_files_writes_substituted_content_create_if_missing() {
    let p = tmp_paths("seed-files");
    let _ = fs::remove_dir_all(&p.base);
    fs::create_dir_all(&p.home).unwrap();
    let spec = IsolationSpec {
        subdir: "test",
        spoof_home: true,
        home_subdirs: &[],
        static_envs: &[],
        seed_files: &[SeedFile {
            path: "{home}/.codex/config.toml",
            content: "home={home}\nanalytics_enabled = false\n",
            overwrite: false,
            mode: None,
        }],
        caveat: None,
    };

    seed_files(&spec, &p).unwrap();
    let path = p.home.join(".codex/config.toml");
    let content = fs::read_to_string(&path).unwrap();

    assert!(content.contains("analytics_enabled = false"));
    assert!(content.contains(&p.home.to_string_lossy().to_string()));

    fs::write(&path, "USER_EDIT").unwrap();
    seed_files(&spec, &p).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "USER_EDIT");
    let _ = fs::remove_dir_all(&p.base);
}

#[test]
fn seed_files_can_overwrite_and_chmod() {
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    let p = tmp_paths("seed-overwrite");
    let _ = fs::remove_dir_all(&p.base);
    fs::create_dir_all(&p.state).unwrap();
    let spec = IsolationSpec {
        subdir: "test",
        spoof_home: true,
        home_subdirs: &[],
        static_envs: &[],
        seed_files: &[SeedFile {
            path: "{state}/apikey.sh",
            content: "#!/bin/sh\nexec hm secret get claude-api-key\n",
            overwrite: true,
            mode: Some(0o700),
        }],
        caveat: None,
    };
    let path = p.state.join("apikey.sh");
    fs::write(&path, "OLD").unwrap();
    #[cfg(unix)]
    let before_inode = {
        use std::os::unix::fs::MetadataExt;
        fs::metadata(&path).unwrap().ino()
    };

    seed_files(&spec, &p).unwrap();

    assert!(fs::read_to_string(&path)
        .unwrap()
        .contains("claude-api-key"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_ne!(fs::metadata(&path).unwrap().ino(), before_inode);
    }
    #[cfg(unix)]
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let _ = fs::remove_dir_all(&p.base);
}

#[test]
fn seed_files_rejects_parent_dir_escape() {
    let p = tmp_paths("seed-escape");
    let _ = fs::remove_dir_all(&p.base);
    fs::create_dir_all(&p.home).unwrap();
    let spec = IsolationSpec {
        subdir: "test",
        spoof_home: true,
        home_subdirs: &[],
        static_envs: &[],
        seed_files: &[SeedFile {
            path: "{home}/../escaped.toml",
            content: "escaped = true\n",
            overwrite: false,
            mode: None,
        }],
        caveat: None,
    };

    let result = seed_files(&spec, &p);

    assert!(result.is_err(), "seed path must not escape isolation base");
    let _ = fs::remove_dir_all(&p.base);
}

#[cfg(unix)]
#[test]
fn seed_files_rejects_existing_seed_symlink() {
    let p = tmp_paths("seed-symlink");
    let _ = fs::remove_dir_all(&p.base);
    fs::create_dir_all(p.home.join(".codex")).unwrap();
    let outside = p.base.join("outside.toml");
    let target = p.home.join(".codex/config.toml");
    std::os::unix::fs::symlink(&outside, &target).unwrap();
    let spec = IsolationSpec {
        subdir: "test",
        spoof_home: true,
        home_subdirs: &[],
        static_envs: &[],
        seed_files: &[SeedFile {
            path: "{home}/.codex/config.toml",
            content: "should-not-write\n",
            overwrite: true,
            mode: None,
        }],
        caveat: None,
    };

    let result = seed_files(&spec, &p);

    assert!(result.is_err());
    assert!(!outside.exists());
    let _ = fs::remove_dir_all(&p.base);
}

#[cfg(unix)]
#[test]
fn seed_files_rejects_symlinked_target() {
    use std::os::unix::fs::symlink;

    let p = tmp_paths("seed-symlink-target");
    let outside = tmp_paths("seed-symlink-outside");
    let _ = fs::remove_dir_all(&p.base);
    let _ = fs::remove_dir_all(&outside.base);
    fs::create_dir_all(&p.state).unwrap();
    fs::create_dir_all(&outside.state).unwrap();
    let outside_target = outside.state.join("real.sh");
    fs::write(&outside_target, "outside").unwrap();
    symlink(&outside_target, p.state.join("apikey.sh")).unwrap();
    let spec = IsolationSpec {
        subdir: "test",
        spoof_home: true,
        home_subdirs: &[],
        static_envs: &[],
        seed_files: &[SeedFile {
            path: "{state}/apikey.sh",
            content: "inside\n",
            overwrite: true,
            mode: Some(0o700),
        }],
        caveat: None,
    };

    let result = seed_files(&spec, &p);

    assert!(result.is_err(), "symlinked seed target must be rejected");
    assert_eq!(fs::read_to_string(outside_target).unwrap(), "outside");
    let _ = fs::remove_dir_all(&p.base);
    let _ = fs::remove_dir_all(&outside.base);
}
