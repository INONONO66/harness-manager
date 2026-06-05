#![allow(dead_code)]

use crate::runtimes::types::{IsolationSpec, SeedFile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsolationPlan {
    pub subdir: String,
    pub spoof_home: bool,
    pub home_subdirs: Vec<String>,
    pub static_envs: Vec<(String, String)>,
    pub seed_files: Vec<SeedFilePlan>,
    pub caveat: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedFilePlan {
    pub path: String,
    pub content: String,
    pub overwrite: bool,
    pub mode: Option<u32>,
}

impl IsolationPlan {
    pub fn from_runtime(spec: &IsolationSpec) -> Self {
        Self {
            subdir: spec.subdir.to_string(),
            spoof_home: spec.spoof_home,
            home_subdirs: spec
                .home_subdirs
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            static_envs: spec
                .static_envs
                .iter()
                .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
                .collect(),
            seed_files: spec
                .seed_files
                .iter()
                .map(SeedFilePlan::from_runtime)
                .collect(),
            caveat: spec.caveat.map(str::to_string),
        }
    }
}

impl SeedFilePlan {
    fn from_runtime(seed: &SeedFile) -> Self {
        Self {
            path: seed.path.to_string(),
            content: seed.content.to_string(),
            overwrite: seed.overwrite,
            mode: seed.mode,
        }
    }
}
