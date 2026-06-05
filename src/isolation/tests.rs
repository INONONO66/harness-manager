use super::IsolationPaths;

mod env_tests;
mod lock_tests;
mod path_tests;
mod seed_tests;
mod token_tests;

fn tmp_paths(suffix: &str) -> IsolationPaths {
    let root = std::env::temp_dir().join(format!(
        "hm-iso-test-{}-{}-{}",
        std::process::id(),
        suffix,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let base = root.join("hm/runtimes/test");
    IsolationPaths {
        home: base.join("home"),
        state: base.join("state"),
        tmp: base.join("tmp"),
        runtime_home: base.join("home"),
        runtime_state: base.join("state"),
        runtime_logs: base.join("state/logs"),
        runtime_base: base.clone(),
        base,
    }
}
