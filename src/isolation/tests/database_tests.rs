use std::fs;

use crate::isolation::{
    ensure_isolation_tree, prepare_runtime_shared_state_from_home, prepare_shared_state_from_home,
};
use crate::runtimes::manifest::SharedStatePlan;

use super::{iso_plan, tmp_paths};

#[cfg(unix)]
fn assert_link_points_at(link: &std::path::Path, target: &std::path::Path) {
    let actual = fs::read_link(link)
        .unwrap_or_else(|err| panic!("expected {} to be a DB symlink: {err}", link.display()));
    assert_eq!(actual, target, "link should point at main runtime DB");
}

#[cfg(unix)]
fn assert_shared_link_points_at(link: &std::path::Path, target: &std::path::Path) {
    let actual = fs::read_link(link)
        .unwrap_or_else(|err| panic!("expected {} to be a shared symlink: {err}", link.display()));
    assert_eq!(
        actual, target,
        "shared link should point at main runtime file"
    );
}

fn shared_state(database_dirs: &[&str], auth_files: &[&str]) -> SharedStatePlan {
    SharedStatePlan {
        database_dirs: database_dirs.iter().map(|path| path.to_string()).collect(),
        auth_files: auth_files.iter().map(|path| path.to_string()).collect(),
    }
}

#[cfg(unix)]
#[test]
fn opencode_nested_db_files_link_to_main_runtime_home() {
    // Given: a main-user OpenCode runtime DB below a session/log subdirectory.
    let paths = tmp_paths("opencode-nested-db");
    let main_home = paths
        .base
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("main-home");
    let source_dir = main_home.join(".local/share/opencode/session/logs");
    fs::create_dir_all(&source_dir).unwrap();
    let source_db = source_dir.join("events.db");
    let source_wal = source_dir.join("events.db-wal");
    fs::write(&source_db, "db").unwrap();
    fs::write(&source_wal, "wal").unwrap();
    fs::write(source_dir.join("notes.txt"), "not a database").unwrap();
    let spec = iso_plan("omo", true, &[], &[], Vec::new(), None);

    // When: the harness isolation tree is prepared for an OpenCode harness.
    ensure_isolation_tree(&spec, &paths).unwrap();
    let plan = shared_state(
        &[".local/share/opencode"],
        &[".local/share/opencode/auth.json"],
    );
    prepare_runtime_shared_state_from_home(Some(&plan), &paths, &main_home).unwrap();

    // Then: nested DB files are symlinked to the pristine main runtime DB.
    let target_dir = paths.home.join(".local/share/opencode/session/logs");
    assert_link_points_at(&target_dir.join("events.db"), &source_db);
    assert_link_points_at(&target_dir.join("events.db-wal"), &source_wal);
    assert!(
        !target_dir.join("notes.txt").exists(),
        "non-DB files must stay isolated"
    );
    let _ = fs::remove_dir_all(paths.base.parent().unwrap().parent().unwrap());
}

#[cfg(unix)]
#[test]
fn database_link_does_not_traverse_symlinked_main_runtime_dirs() {
    // Given: a main runtime DB tree contains a symlinked directory outside that tree.
    let paths = tmp_paths("source-symlink-db");
    let main_home = paths
        .base
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("main-home");
    let external_dir = paths
        .base
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("external-db");
    fs::create_dir_all(&external_dir).unwrap();
    let external_db = external_dir.join("escaped.db");
    fs::write(&external_db, "outside").unwrap();
    let source_dir = main_home.join(".local/share/opencode");
    fs::create_dir_all(&source_dir).unwrap();
    std::os::unix::fs::symlink(&external_dir, source_dir.join("linked-out")).unwrap();
    let spec = iso_plan("omo", true, &[], &[], Vec::new(), None);

    // When: the harness isolation tree links OpenCode DB files.
    ensure_isolation_tree(&spec, &paths).unwrap();
    let plan = shared_state(
        &[".local/share/opencode"],
        &[".local/share/opencode/auth.json"],
    );
    prepare_runtime_shared_state_from_home(Some(&plan), &paths, &main_home).unwrap();

    // Then: DB discovery stays inside the real main runtime tree.
    assert!(
        !paths
            .home
            .join(".local/share/opencode/linked-out/escaped.db")
            .exists(),
        "source symlink directories must not be traversed"
    );
    let _ = fs::remove_dir_all(paths.base.parent().unwrap().parent().unwrap());
}

#[cfg(unix)]
#[test]
fn database_link_rejects_existing_harness_local_db() {
    // Given: a harness already has a local DB where the main runtime DB should link.
    let paths = tmp_paths("existing-local-db");
    let main_home = paths
        .base
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("main-home");
    let source_dir = main_home.join(".codex/sessions");
    fs::create_dir_all(&source_dir).unwrap();
    fs::write(source_dir.join("events.sqlite"), "main").unwrap();
    let spec = iso_plan("omx", true, &[".codex"], &[], Vec::new(), None);
    ensure_isolation_tree(&spec, &paths).unwrap();
    let local_db = paths.home.join(".codex/sessions/events.sqlite");
    fs::create_dir_all(local_db.parent().unwrap()).unwrap();
    fs::write(&local_db, "local").unwrap();

    // When: core tries to link the main runtime DB into the harness home.
    let plan = shared_state(&[".codex"], &[".codex/auth.json"]);
    let err = prepare_runtime_shared_state_from_home(Some(&plan), &paths, &main_home).unwrap_err();

    // Then: launch is blocked instead of silently forking session state.
    assert!(
        err.to_string().contains("already exists"),
        "expected existing local DB error, got: {err:#}"
    );
    let _ = fs::remove_dir_all(paths.base.parent().unwrap().parent().unwrap());
}

#[cfg(unix)]
#[test]
fn allowlisted_auth_files_link_to_main_runtime_home() {
    let cases = [
        ("codex-auth", ".codex/auth.json"),
        ("claude-auth", ".claude/.credentials.json"),
        ("opencode-auth", ".local/share/opencode/auth.json"),
        ("pi-auth", ".pi/agent/auth.json"),
    ];

    for (case_name, auth_relative) in cases {
        let paths = tmp_paths(case_name);
        let main_home = paths
            .base
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("main-home");
        let source = main_home.join(auth_relative);
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        fs::write(&source, "token").unwrap();
        let spec = iso_plan(case_name, true, &[], &[], Vec::new(), None);

        ensure_isolation_tree(&spec, &paths).unwrap();
        let plan = shared_state(&[], &[auth_relative]);
        prepare_runtime_shared_state_from_home(Some(&plan), &paths, &main_home).unwrap();

        assert_shared_link_points_at(&paths.home.join(auth_relative), &source);
        let _ = fs::remove_dir_all(paths.base.parent().unwrap().parent().unwrap());
    }
}

#[cfg(unix)]
#[test]
fn auth_link_rejects_existing_harness_local_auth_file() {
    let paths = tmp_paths("existing-local-auth");
    let main_home = paths
        .base
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("main-home");
    let source = main_home.join(".codex/auth.json");
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    fs::write(&source, "main").unwrap();
    let spec = iso_plan("omx-auth", true, &[".codex"], &[], Vec::new(), None);
    ensure_isolation_tree(&spec, &paths).unwrap();
    let local_auth = paths.home.join(".codex/auth.json");
    fs::write(&local_auth, "local").unwrap();

    let plan = shared_state(&[".codex"], &[".codex/auth.json"]);
    let err = prepare_runtime_shared_state_from_home(Some(&plan), &paths, &main_home).unwrap_err();

    assert!(
        err.to_string().contains("already exists"),
        "expected existing local auth error, got: {err:#}"
    );
    let _ = fs::remove_dir_all(paths.base.parent().unwrap().parent().unwrap());
}

#[cfg(unix)]
#[test]
fn manifest_declared_shared_state_links_auth_and_database_files() {
    let paths = tmp_paths("manifest-shared-state");
    let main_home = paths
        .base
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("main-home");
    let db_dir = main_home.join(".custom/data/session");
    fs::create_dir_all(&db_dir).unwrap();
    let source_db = db_dir.join("events.sqlite");
    fs::write(&source_db, "db").unwrap();
    fs::write(db_dir.join("notes.txt"), "not a database").unwrap();
    let source_auth = main_home.join(".custom/auth/token.json");
    fs::create_dir_all(source_auth.parent().unwrap()).unwrap();
    fs::write(&source_auth, "token").unwrap();
    let plan = SharedStatePlan {
        database_dirs: vec![".custom/data".to_string()],
        auth_files: vec![".custom/auth/token.json".to_string()],
    };
    let spec = iso_plan("custom-shared", true, &[], &[], Vec::new(), None);

    ensure_isolation_tree(&spec, &paths).unwrap();
    prepare_shared_state_from_home(&plan, &paths, &main_home).unwrap();

    assert_shared_link_points_at(
        &paths.home.join(".custom/data/session/events.sqlite"),
        &source_db,
    );
    assert!(
        !paths.home.join(".custom/data/session/notes.txt").exists(),
        "non-database files must stay isolated"
    );
    assert_shared_link_points_at(&paths.home.join(".custom/auth/token.json"), &source_auth);
    let _ = fs::remove_dir_all(paths.base.parent().unwrap().parent().unwrap());
}
