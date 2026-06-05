use std::fs;

use crate::isolation::{IsolationLockGuard, IsolationPaths};

use super::tmp_paths;

#[test]
fn isolation_lock_file_lives_under_runtimes_lock_dir() {
    let paths = tmp_paths("lock-path");

    let lock_file = paths.lock_file().unwrap();

    assert_eq!(
        lock_file,
        paths
            .base
            .parent()
            .unwrap()
            .join(".locks")
            .join("test.lock")
    );
}

#[test]
fn harnesses_targeting_same_runtime_share_lock_file() {
    let root = test_root("shared-codex-lock");
    let first = shared_runtime_paths_under(&root, "lazycodex-lock", "codex");
    let second = shared_runtime_paths_under(&root, "omx-lock", "codex");

    assert_eq!(first.lock_file().unwrap(), second.lock_file().unwrap());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn harnesses_targeting_different_runtimes_use_different_lock_files() {
    let root = test_root("different-runtime-locks");
    let codex = shared_runtime_paths_under(&root, "lazycodex-lock", "codex");
    let opencode = shared_runtime_paths_under(&root, "omo-lock", "opencode");

    assert_ne!(codex.lock_file().unwrap(), opencode.lock_file().unwrap());
    let _ = fs::remove_dir_all(root);
}

#[test]
fn isolation_lock_serializes_second_acquirer() {
    use std::sync::mpsc;
    use std::time::Duration;

    let paths = tmp_paths("lock-serializes");
    let first = IsolationLockGuard::acquire(&paths).unwrap();
    let second_paths = paths.clone();
    let (sender, receiver) = mpsc::channel();

    let handle = std::thread::spawn(move || {
        let _second = IsolationLockGuard::acquire(&second_paths).unwrap();
        sender.send(()).unwrap();
    });

    assert!(
        receiver.recv_timeout(Duration::from_millis(100)).is_err(),
        "second acquirer must block while first lock is held"
    );
    drop(first);
    receiver
        .recv_timeout(Duration::from_secs(2))
        .expect("second acquirer should proceed after first lock drops");
    handle.join().unwrap();
    let _ = fs::remove_dir_all(paths.base.parent().unwrap().parent().unwrap());
}

fn shared_runtime_paths_under(
    root: &std::path::Path,
    harness_subdir: &str,
    runtime_subdir: &str,
) -> IsolationPaths {
    let base = root.join("hm/runtimes").join(harness_subdir);
    let runtime_base = root.join("hm/runtimes").join(runtime_subdir);
    IsolationPaths {
        home: base.join("home"),
        state: base.join("state"),
        tmp: base.join("tmp"),
        runtime_home: runtime_base.join("home"),
        runtime_state: runtime_base.join("state"),
        runtime_logs: runtime_base.join("state/logs"),
        runtime_base,
        base,
    }
}

fn test_root(suffix: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "hm-iso-test-{}-{}-{}",
        std::process::id(),
        suffix,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}
