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
        session_dirs: Vec::new(),
        session_files: Vec::new(),
        session_dir_globs: Vec::new(),
        session_file_globs: Vec::new(),
        auth_files: auth_files.iter().map(|path| path.to_string()).collect(),
    }
}

fn shared_state_with_sessions(
    database_dirs: &[&str],
    session_dirs: &[&str],
    session_files: &[&str],
    auth_files: &[&str],
) -> SharedStatePlan {
    SharedStatePlan {
        database_dirs: database_dirs.iter().map(|path| path.to_string()).collect(),
        session_dirs: session_dirs.iter().map(|path| path.to_string()).collect(),
        session_files: session_files.iter().map(|path| path.to_string()).collect(),
        session_dir_globs: Vec::new(),
        session_file_globs: Vec::new(),
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
    prepare_runtime_shared_state_from_home(Some(&plan), &paths, &main_home, true).unwrap();

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
    prepare_runtime_shared_state_from_home(Some(&plan), &paths, &main_home, true).unwrap();

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
    let err =
        prepare_runtime_shared_state_from_home(Some(&plan), &paths, &main_home, true).unwrap_err();

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
        prepare_runtime_shared_state_from_home(Some(&plan), &paths, &main_home, true).unwrap();

        assert_shared_link_points_at(&paths.home.join(auth_relative), &source);
        let _ = fs::remove_dir_all(paths.base.parent().unwrap().parent().unwrap());
    }
}

#[cfg(unix)]
#[test]
fn auth_files_are_not_shared_when_launch_uses_profile_credentials() {
    let paths = tmp_paths("profile-seeded-auth");
    let main_home = paths
        .base
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("main-home");
    let source_auth = main_home.join(".codex/auth.json");
    let source_db_dir = main_home.join(".codex/sessions");
    let source_db = source_db_dir.join("events.sqlite");
    fs::create_dir_all(source_auth.parent().unwrap()).unwrap();
    fs::create_dir_all(&source_db_dir).unwrap();
    fs::write(&source_auth, "token").unwrap();
    fs::write(&source_db, "db").unwrap();
    let spec = iso_plan(
        "profile-seeded-auth",
        true,
        &[".codex"],
        &[],
        Vec::new(),
        None,
    );

    ensure_isolation_tree(&spec, &paths).unwrap();
    let plan = shared_state(&[".codex"], &[".codex/auth.json"]);
    prepare_runtime_shared_state_from_home(Some(&plan), &paths, &main_home, false).unwrap();

    assert!(
        !paths.home.join(".codex/auth.json").exists(),
        "profile-driven launches must not link host auth"
    );
    assert_shared_link_points_at(
        &paths.home.join(".codex/sessions/events.sqlite"),
        &source_db,
    );
    let _ = fs::remove_dir_all(paths.base.parent().unwrap().parent().unwrap());
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
    let err =
        prepare_runtime_shared_state_from_home(Some(&plan), &paths, &main_home, true).unwrap_err();

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
        session_dirs: Vec::new(),
        session_files: Vec::new(),
        session_dir_globs: Vec::new(),
        session_file_globs: Vec::new(),
        auth_files: vec![".custom/auth/token.json".to_string()],
    };
    let spec = iso_plan("custom-shared", true, &[], &[], Vec::new(), None);

    ensure_isolation_tree(&spec, &paths).unwrap();
    prepare_shared_state_from_home(&plan, &paths, &main_home, true).unwrap();

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

#[cfg(unix)]
#[test]
fn session_only_sharing_links_session_dirs_and_files_into_isolated_home() {
    // Given: a main runtime home with JSONL session state and an isolated wrapper home.
    let mut paths = tmp_paths("session-only-sharing");
    paths.runtime_home = paths.base.join("native-runtime-home");
    let main_home = paths
        .base
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("main-home");
    let source_session_dir = main_home.join(".codex/sessions/2026/06/21");
    let source_archive_dir = main_home.join(".codex/archived_sessions/2026/06/20");
    let source_history = main_home.join(".codex/history.jsonl");
    let source_config = main_home.join(".codex/config.toml");
    fs::create_dir_all(&source_session_dir).unwrap();
    fs::create_dir_all(&source_archive_dir).unwrap();
    fs::write(source_session_dir.join("rollout-1.jsonl"), "session").unwrap();
    fs::write(source_archive_dir.join("rollout-old.jsonl"), "archived").unwrap();
    fs::write(&source_history, "history").unwrap();
    fs::write(&source_config, "config must not link").unwrap();
    let spec = iso_plan("session-only-sharing", true, &[], &[], Vec::new(), None);

    // When: session-only shared state is prepared.
    ensure_isolation_tree(&spec, &paths).unwrap();
    let plan = shared_state_with_sessions(
        &[],
        &[".codex/sessions", ".codex/archived_sessions"],
        &[".codex/history.jsonl"],
        &[],
    );
    prepare_runtime_shared_state_from_home(Some(&plan), &paths, &main_home, false).unwrap();

    // Then: session artifacts are linked into the wrapper's isolated home, while config is not.
    assert_shared_link_points_at(
        &paths.home.join(".codex/sessions"),
        &main_home.join(".codex/sessions"),
    );
    assert_shared_link_points_at(
        &paths.home.join(".codex/archived_sessions"),
        &main_home.join(".codex/archived_sessions"),
    );
    assert_shared_link_points_at(&paths.home.join(".codex/history.jsonl"), &source_history);
    assert!(
        !paths.home.join(".codex/config.toml").exists(),
        "session-only sharing must not link runtime config"
    );
    assert!(
        !paths
            .runtime_home
            .join(".codex/sessions/2026/06/21/rollout-1.jsonl")
            .exists(),
        "shared session state must not target the shared runtime home"
    );
    let _ = fs::remove_dir_all(paths.base.parent().unwrap().parent().unwrap());
}

#[cfg(unix)]
#[test]
fn session_sharing_replaces_empty_isolated_session_stub() {
    let paths = tmp_paths("session-stub-relink");
    let main_home = paths
        .base
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("main-home");
    let source_session_dir = main_home.join(".local/share/opencode/storage/session_diff");
    fs::create_dir_all(&source_session_dir).unwrap();
    let source_session = source_session_dir.join("ses_123.json");
    fs::write(&source_session, "{}").unwrap();

    ensure_isolation_tree(&iso_plan("omo", true, &[], &[], Vec::new(), None), &paths).unwrap();
    let local_session = paths
        .home
        .join(".local/share/opencode/storage/session_diff/ses_123.json");
    fs::create_dir_all(local_session.parent().unwrap()).unwrap();
    fs::write(&local_session, "{}").unwrap();

    let plan = shared_state_with_sessions(
        &[],
        &[".local/share/opencode/storage/session_diff"],
        &[],
        &[],
    );
    prepare_runtime_shared_state_from_home(Some(&plan), &paths, &main_home, false).unwrap();

    assert_shared_link_points_at(
        &paths
            .home
            .join(".local/share/opencode/storage/session_diff"),
        &source_session_dir,
    );
    assert_eq!(
        fs::read_to_string(&source_session).unwrap(),
        "{}",
        "stale local session stub must not replace the host session file"
    );
    let _ = fs::remove_dir_all(paths.base.parent().unwrap().parent().unwrap());
}

#[cfg(unix)]
#[test]
fn session_sharing_migrates_existing_isolated_session_state_before_linking() {
    let paths = tmp_paths("session-migrate-stale-isolation");
    let main_home = paths
        .base
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("main-home");
    let source_sessions = main_home.join(".codex/sessions");
    let source_history = main_home.join(".codex/history.jsonl");
    fs::create_dir_all(source_sessions.join("2026/06/22")).unwrap();
    fs::write(
        source_sessions.join("2026/06/22/existing.jsonl"),
        "host-session",
    )
    .unwrap();
    fs::write(&source_history, "host-history\n").unwrap();

    ensure_isolation_tree(
        &iso_plan("lazycodex", true, &[], &[], Vec::new(), None),
        &paths,
    )
    .unwrap();
    let local_sessions = paths.home.join(".codex/sessions");
    let local_history = paths.home.join(".codex/history.jsonl");
    fs::create_dir_all(local_sessions.join("2026/06/22")).unwrap();
    fs::write(
        local_sessions.join("2026/06/22/new-local.jsonl"),
        "local-session",
    )
    .unwrap();
    fs::write(
        local_sessions.join("2026/06/22/existing.jsonl"),
        "conflicting-local-session",
    )
    .unwrap();
    fs::write(&local_history, "local-history\n").unwrap();

    let plan =
        shared_state_with_sessions(&[], &[".codex/sessions"], &[".codex/history.jsonl"], &[]);
    prepare_runtime_shared_state_from_home(Some(&plan), &paths, &main_home, false).unwrap();

    assert_shared_link_points_at(&local_sessions, &source_sessions);
    assert_shared_link_points_at(&local_history, &source_history);
    assert_eq!(
        fs::read_to_string(source_sessions.join("2026/06/22/new-local.jsonl")).unwrap(),
        "local-session"
    );
    assert!(
        source_sessions
            .join("2026/06/22/.hm-migrated-1-existing.jsonl")
            .is_file(),
        "conflicting local session file must be preserved beside host session"
    );
    assert!(
        main_home
            .join(".codex/.hm-migrated-1-history.jsonl")
            .is_file(),
        "conflicting local history must be preserved beside host history"
    );
    let _ = fs::remove_dir_all(paths.base.parent().unwrap().parent().unwrap());
}

#[cfg(unix)]
#[test]
fn shared_state_wildcards_link_session_dbs_without_sharing_config() {
    // Given: OpenCode session DB files in fixed and legacy project-scoped locations.
    let paths = tmp_paths("session-wildcards");
    let main_home = paths
        .base
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("main-home");
    let data_dir = main_home.join(".local/share/opencode");
    let project_storage = data_dir.join("project/example/storage");
    fs::create_dir_all(&project_storage).unwrap();
    let source_db = data_dir.join("opencode.db");
    let source_wal = data_dir.join("opencode.db-wal");
    let source_project_db = project_storage.join("session.db");
    fs::write(&source_db, "db").unwrap();
    fs::write(&source_wal, "wal").unwrap();
    fs::write(&source_project_db, "project-db").unwrap();
    fs::write(data_dir.join("opencode.json"), "config must not link").unwrap();
    let spec = iso_plan("omo", true, &[], &[], Vec::new(), None);

    // When: wildcard session shared state is prepared.
    ensure_isolation_tree(&spec, &paths).unwrap();
    let mut plan = shared_state_with_sessions(&[], &[], &[], &[]);
    plan.session_file_globs = vec![".local/share/opencode/opencode.db*".to_string()];
    plan.session_dir_globs = vec![".local/share/opencode/project/*/storage".to_string()];
    prepare_runtime_shared_state_from_home(Some(&plan), &paths, &main_home, false).unwrap();

    // Then: session DB files are linked, but adjacent config remains isolated.
    assert_shared_link_points_at(
        &paths.home.join(".local/share/opencode/opencode.db"),
        &source_db,
    );
    assert_shared_link_points_at(
        &paths.home.join(".local/share/opencode/opencode.db-wal"),
        &source_wal,
    );
    assert_shared_link_points_at(
        &paths
            .home
            .join(".local/share/opencode/project/example/storage"),
        &project_storage,
    );
    assert!(paths
        .home
        .join(".local/share/opencode/project/example/storage/session.db")
        .exists());
    assert!(
        !paths
            .home
            .join(".local/share/opencode/opencode.json")
            .exists(),
        "wildcard session sharing must not link adjacent config"
    );
    let _ = fs::remove_dir_all(paths.base.parent().unwrap().parent().unwrap());
}

#[cfg(unix)]
#[test]
fn session_sharing_rejects_symlinked_source_root() {
    // Given: a declared session root in the host runtime home is itself a symlink.
    let paths = tmp_paths("session-source-root-symlink");
    let test_root = paths.base.parent().unwrap().parent().unwrap().to_path_buf();
    let main_home = test_root.join("main-home");
    let external_sessions = test_root.join("external-sessions");
    fs::create_dir_all(main_home.join(".codex")).unwrap();
    fs::create_dir_all(&external_sessions).unwrap();
    fs::write(external_sessions.join("rollout.jsonl"), "escaped").unwrap();
    std::os::unix::fs::symlink(&external_sessions, main_home.join(".codex/sessions")).unwrap();
    let spec = iso_plan("omx", true, &[], &[], Vec::new(), None);

    // When: shared session state preparation reaches the symlinked source root.
    ensure_isolation_tree(&spec, &paths).unwrap();
    let plan = shared_state_with_sessions(&[], &[".codex/sessions"], &[], &[]);
    let err = prepare_runtime_shared_state_from_home(Some(&plan), &paths, &main_home, false)
        .expect_err("symlinked source roots must be rejected");

    // Then: no escaped file is linked into the isolated runtime home.
    assert!(
        err.to_string().contains("source symlink"),
        "expected source symlink rejection, got: {err:#}"
    );
    assert!(
        !paths.home.join(".codex/sessions/rollout.jsonl").exists(),
        "session sharing must not follow symlinked source roots"
    );
    let _ = fs::remove_dir_all(test_root);
}

#[cfg(unix)]
#[test]
fn session_globs_reject_symlinked_fixed_prefix() {
    // Given: a fixed glob prefix points through a source symlink before wildcard expansion.
    let paths = tmp_paths("session-glob-prefix-symlink");
    let test_root = paths.base.parent().unwrap().parent().unwrap().to_path_buf();
    let main_home = test_root.join("main-home");
    let external_share = test_root.join("external-share");
    fs::create_dir_all(main_home.join(".local/share")).unwrap();
    fs::create_dir_all(external_share.join("opencode/project/example/storage")).unwrap();
    fs::write(
        external_share.join("opencode/project/example/storage/session.db"),
        "escaped",
    )
    .unwrap();
    std::os::unix::fs::symlink(&external_share, main_home.join(".local/share/opencode")).unwrap();
    let spec = iso_plan("omo", true, &[], &[], Vec::new(), None);

    // When: wildcard session sharing reaches the symlinked fixed prefix.
    ensure_isolation_tree(&spec, &paths).unwrap();
    let mut plan = shared_state_with_sessions(&[], &[], &[], &[]);
    plan.session_dir_globs = vec![".local/share/opencode/project/*/storage".to_string()];
    let err = prepare_runtime_shared_state_from_home(Some(&plan), &paths, &main_home, false)
        .expect_err("symlinked glob prefixes must be rejected");

    // Then: no escaped project storage is linked into the isolated runtime home.
    assert!(
        err.to_string().contains("source symlink"),
        "expected source symlink rejection, got: {err:#}"
    );
    assert!(
        !paths
            .home
            .join(".local/share/opencode/project/example/storage/session.db")
            .exists(),
        "glob sharing must not follow symlinked fixed prefixes"
    );
    let _ = fs::remove_dir_all(test_root);
}

#[cfg(unix)]
#[test]
fn session_globs_reject_symlinked_matched_directory() {
    // Given: the wildcard match is real, but the declared storage directory is a symlink.
    let paths = tmp_paths("session-glob-storage-symlink");
    let test_root = paths.base.parent().unwrap().parent().unwrap().to_path_buf();
    let main_home = test_root.join("main-home");
    let project_dir = main_home.join(".local/share/opencode/project/example");
    let external_storage = test_root.join("external-storage");
    fs::create_dir_all(&project_dir).unwrap();
    fs::create_dir_all(&external_storage).unwrap();
    fs::write(external_storage.join("session.db"), "escaped").unwrap();
    std::os::unix::fs::symlink(&external_storage, project_dir.join("storage")).unwrap();
    let spec = iso_plan("omo", true, &[], &[], Vec::new(), None);

    // When: wildcard session sharing reaches the symlinked matched directory.
    ensure_isolation_tree(&spec, &paths).unwrap();
    let mut plan = shared_state_with_sessions(&[], &[], &[], &[]);
    plan.session_dir_globs = vec![".local/share/opencode/project/*/storage".to_string()];
    let err = prepare_runtime_shared_state_from_home(Some(&plan), &paths, &main_home, false)
        .expect_err("symlinked matched directories must be rejected");

    // Then: no escaped project storage is linked into the isolated runtime home.
    assert!(
        err.to_string().contains("source symlink"),
        "expected source symlink rejection, got: {err:#}"
    );
    assert!(
        !paths
            .home
            .join(".local/share/opencode/project/example/storage/session.db")
            .exists(),
        "glob sharing must not follow symlinked matched directories"
    );
    let _ = fs::remove_dir_all(test_root);
}
