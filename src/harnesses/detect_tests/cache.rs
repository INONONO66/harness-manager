use super::*;

#[test]
fn package_cache_installed_at_finds_npx_node_modules_one_hash_deep() {
    let tmp = tempfile::tempdir().unwrap();
    let pkg = "test-only-npx-pkg";
    let leaf = tmp
        .path()
        .join(".npm")
        .join("_npx")
        .join("abcdef0123")
        .join("node_modules")
        .join(pkg);
    std::fs::create_dir_all(&leaf).unwrap();

    let spec = PackageSpec::NpxInstaller {
        package: pkg.to_string(),
        args: Vec::new(),
        self_update: None,
    };
    let result = package_cache_installed_at(tmp.path(), &spec);

    let path = result.expect("npx cache lookup finds seeded package");
    assert!(path.ends_with(format!("node_modules/{pkg}")));
}

#[test]
fn package_cache_installed_at_returns_none_when_no_npx_cache() {
    let tmp = tempfile::tempdir().unwrap();
    let spec = PackageSpec::NpxInstaller {
        package: "ghost-pkg".to_string(),
        args: Vec::new(),
        self_update: None,
    };
    assert!(package_cache_installed_at(tmp.path(), &spec).is_none());
}

#[test]
fn package_cache_installed_at_finds_bunx_cache_versioned_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let pkg = "test-only-bunx-pkg";
    let entry = tmp
        .path()
        .join(".bun")
        .join("install")
        .join("cache")
        .join(format!("{pkg}@2.0.1"));
    std::fs::create_dir_all(&entry).unwrap();

    let spec = PackageSpec::BunxInstaller {
        package: pkg.to_string(),
        args: Vec::new(),
        self_update: None,
    };
    let result = package_cache_installed_at(tmp.path(), &spec);
    assert!(result
        .expect("bunx cache lookup hit")
        .ends_with(format!(".bun/install/cache/{pkg}@2.0.1")));
}

#[test]
fn package_cache_installed_at_finds_bunx_cache_bare_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let pkg = "test-only-bunx-pkg";
    let bare = tmp
        .path()
        .join(".bun")
        .join("install")
        .join("cache")
        .join(pkg);
    std::fs::create_dir_all(&bare).unwrap();

    let spec = PackageSpec::BunxInstaller {
        package: pkg.to_string(),
        args: Vec::new(),
        self_update: None,
    };
    let result = package_cache_installed_at(tmp.path(), &spec);
    assert!(result.is_some());
}

#[test]
fn package_cache_installed_at_finds_bunx_xdg_cache_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let pkg = "test-only-bunx-pkg";
    let entry = tmp
        .path()
        .join(".cache")
        .join(".bun")
        .join("install")
        .join("cache")
        .join(format!("{pkg}@2.0.1@@@1"));
    std::fs::create_dir_all(&entry).unwrap();

    let spec = PackageSpec::BunxInstaller {
        package: pkg.to_string(),
        args: Vec::new(),
        self_update: None,
    };
    let result = package_cache_installed_at(tmp.path(), &spec);

    assert!(result
        .expect("bunx XDG cache lookup hit")
        .ends_with(format!(".cache/.bun/install/cache/{pkg}@2.0.1@@@1")));
}

#[test]
fn package_cache_installed_at_ignores_unrelated_bunx_cache_entry() {
    let tmp = tempfile::tempdir().unwrap();
    let unrelated = tmp
        .path()
        .join(".bun")
        .join("install")
        .join("cache")
        .join("some-other-package@1.0.0");
    std::fs::create_dir_all(&unrelated).unwrap();

    let spec = PackageSpec::BunxInstaller {
        package: "test-only-bunx-pkg".to_string(),
        args: Vec::new(),
        self_update: None,
    };
    assert!(package_cache_installed_at(tmp.path(), &spec).is_none());
}

#[test]
fn package_cache_installed_at_returns_none_for_non_installer_kinds() {
    let tmp = tempfile::tempdir().unwrap();
    for spec in [
        PackageSpec::NpmGlobal {
            package: "p".to_string(),
            self_update: None,
        },
        PackageSpec::NpmIsolated {
            package: "p".to_string(),
            self_update: None,
        },
        PackageSpec::PythonTool {
            package: "p".to_string(),
            self_update: None,
        },
        PackageSpec::Manual {
            instructions: "x".to_string(),
            self_update: None,
        },
    ] {
        assert!(
            package_cache_installed_at(tmp.path(), &spec).is_none(),
            "{:?} must not match installer cache",
            spec
        );
    }
}
