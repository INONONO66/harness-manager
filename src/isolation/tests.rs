use super::spec::{IsolationPlan, SeedFilePlan};
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

fn iso_plan(
    subdir: &str,
    spoof_home: bool,
    home_subdirs: &[&str],
    static_envs: &[(&str, &str)],
    seed_files: Vec<SeedFilePlan>,
    caveat: Option<&str>,
) -> IsolationPlan {
    IsolationPlan {
        subdir: subdir.to_string(),
        runtime_subdir: subdir.to_string(),
        spoof_home,
        home_subdirs: home_subdirs.iter().map(|s| s.to_string()).collect(),
        static_envs: static_envs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        seed_files,
        caveat: caveat.map(|s| s.to_string()),
    }
}

fn seed(path: &str, content: &str, overwrite: bool, mode: Option<u32>) -> SeedFilePlan {
    SeedFilePlan {
        path: path.to_string(),
        content: content.to_string(),
        overwrite,
        mode,
    }
}
