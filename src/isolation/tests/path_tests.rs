use std::fs;

use crate::isolation::{ensure_isolation_tree, purge_isolation_tree, IsolationPaths};
use crate::runtimes::types::IsolationSpec;

use super::tmp_paths;

#[test]
fn try_from_spec_rejects_parent_dir_runtime_subdir() {
    let spec = IsolationSpec {
        subdir: "../codex",
        spoof_home: true,
        home_subdirs: &[".codex"],
        static_envs: &[],
        seed_files: &[],
        caveat: None,
    };

    let result = IsolationPaths::try_from_spec(&spec);

    assert!(
        result.is_err(),
        "isolation subdir must not escape $HM/runtimes"
    );
}

#[test]
fn ensure_tree_rejects_absolute_home_subdir() {
    let p = tmp_paths("absolute-home-subdir");
    let _ = fs::remove_dir_all(&p.base);
    let spec = IsolationSpec {
        subdir: "test",
        spoof_home: true,
        home_subdirs: &["/tmp/escaped"],
        static_envs: &[],
        seed_files: &[],
        caveat: None,
    };

    let result = ensure_isolation_tree(&spec, &p);

    assert!(result.is_err(), "absolute home subdir must be rejected");
    let _ = fs::remove_dir_all(&p.base);
}

#[cfg(unix)]
#[test]
fn ensure_tree_rejects_symlinked_home() {
    use std::os::unix::fs::symlink;

    let p = tmp_paths("symlink-home");
    let outside = tmp_paths("symlink-home-outside");
    let _ = fs::remove_dir_all(&p.base);
    let _ = fs::remove_dir_all(&outside.base);
    fs::create_dir_all(&p.base).unwrap();
    fs::create_dir_all(&outside.home).unwrap();
    symlink(&outside.home, &p.home).unwrap();
    let spec = IsolationSpec {
        subdir: "test",
        spoof_home: true,
        home_subdirs: &[],
        static_envs: &[],
        seed_files: &[],
        caveat: None,
    };

    let result = ensure_isolation_tree(&spec, &p);

    assert!(result.is_err(), "symlinked isolation home must be rejected");
    let _ = fs::remove_dir_all(&p.base);
    let _ = fs::remove_dir_all(&outside.base);
}

#[cfg(unix)]
#[test]
fn ensure_tree_rejects_symlinked_runtimes_ancestor() {
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!(
        "hm-iso-test-runtimes-symlink-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let hm_root = root.join("xdg/hm");
    let outside = root.join("outside");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&hm_root).unwrap();
    fs::create_dir_all(&outside).unwrap();
    symlink(&outside, hm_root.join("runtimes")).unwrap();
    let p = IsolationPaths {
        base: hm_root.join("runtimes/sample-harness"),
        home: hm_root.join("runtimes/sample-harness/home"),
        state: hm_root.join("runtimes/sample-harness/state"),
        tmp: hm_root.join("runtimes/sample-harness/tmp"),
    };
    let spec = IsolationSpec {
        subdir: "sample-harness",
        spoof_home: true,
        home_subdirs: &[".codex"],
        static_envs: &[],
        seed_files: &[],
        caveat: None,
    };

    let result = ensure_isolation_tree(&spec, &p);

    assert!(
        result.is_err(),
        "symlinked runtimes ancestor must be rejected"
    );
    assert!(
        !outside.join("sample-harness").exists(),
        "isolation tree must not be created through ancestor symlink"
    );
    let _ = fs::remove_dir_all(&root);
}

#[cfg(unix)]
#[test]
fn purge_rejects_symlinked_runtimes_ancestor() {
    use std::os::unix::fs::symlink;

    let root = std::env::temp_dir().join(format!(
        "hm-iso-test-purge-runtimes-symlink-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let hm_root = root.join("xdg/hm");
    let outside = root.join("outside");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&hm_root).unwrap();
    fs::create_dir_all(outside.join("sample-harness")).unwrap();
    fs::write(outside.join("sample-harness/sentinel"), "outside").unwrap();
    symlink(&outside, hm_root.join("runtimes")).unwrap();
    let p = IsolationPaths {
        base: hm_root.join("runtimes/sample-harness"),
        home: hm_root.join("runtimes/sample-harness/home"),
        state: hm_root.join("runtimes/sample-harness/state"),
        tmp: hm_root.join("runtimes/sample-harness/tmp"),
    };

    let result = purge_isolation_tree(&p);

    assert!(
        result.is_err(),
        "purge must reject symlinked runtimes ancestor"
    );
    assert!(
        outside.join("sample-harness/sentinel").exists(),
        "purge must not delete through ancestor symlink"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn all_registered_harnesses_have_distinct_isolation_roots() {
    let mut roots = std::collections::HashSet::new();

    let registry = crate::harnesses::registry::HarnessRegistry::builtin_only().unwrap();
    for harness in registry.specs() {
        let paths = IsolationPaths::try_from_spec(&harness.isolation).unwrap();
        assert!(
            roots.insert(paths.base.clone()),
            "duplicate isolation root for {}",
            harness.id
        );
        assert!(
            paths.base.ends_with(&harness.isolation.subdir),
            "root should end with harness subdir for {}",
            harness.id
        );
    }
}
