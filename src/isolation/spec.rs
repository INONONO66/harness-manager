use crate::runtimes::types::{IsolationSpec, SeedFile};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeedFileView<'a> {
    pub path: &'a str,
    pub content: &'a str,
    pub overwrite: bool,
    pub mode: Option<u32>,
}

pub trait IsolationRecipe {
    fn subdir(&self) -> &str;
    fn runtime_subdir(&self) -> &str {
        self.subdir()
    }
    fn spoof_home(&self) -> bool;
    fn home_subdirs(&self) -> Vec<&str>;
    fn static_envs(&self) -> Vec<(&str, &str)>;
    fn seed_files(&self) -> Vec<SeedFileView<'_>>;
    fn caveat(&self) -> Option<&str>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsolationPlan {
    pub subdir: String,
    pub runtime_subdir: String,
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
            runtime_subdir: spec.subdir.to_string(),
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

impl IsolationRecipe for IsolationSpec {
    fn subdir(&self) -> &str {
        self.subdir
    }

    fn spoof_home(&self) -> bool {
        self.spoof_home
    }

    fn home_subdirs(&self) -> Vec<&str> {
        self.home_subdirs.to_vec()
    }

    fn static_envs(&self) -> Vec<(&str, &str)> {
        self.static_envs.to_vec()
    }

    fn seed_files(&self) -> Vec<SeedFileView<'_>> {
        self.seed_files
            .iter()
            .map(|seed| SeedFileView {
                path: seed.path,
                content: seed.content,
                overwrite: seed.overwrite,
                mode: seed.mode,
            })
            .collect()
    }

    fn caveat(&self) -> Option<&str> {
        self.caveat
    }
}

impl IsolationRecipe for IsolationPlan {
    fn subdir(&self) -> &str {
        &self.subdir
    }

    fn runtime_subdir(&self) -> &str {
        &self.runtime_subdir
    }

    fn spoof_home(&self) -> bool {
        self.spoof_home
    }

    fn home_subdirs(&self) -> Vec<&str> {
        self.home_subdirs.iter().map(String::as_str).collect()
    }

    fn static_envs(&self) -> Vec<(&str, &str)> {
        self.static_envs
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect()
    }

    fn seed_files(&self) -> Vec<SeedFileView<'_>> {
        self.seed_files
            .iter()
            .map(|seed| SeedFileView {
                path: &seed.path,
                content: &seed.content,
                overwrite: seed.overwrite,
                mode: seed.mode,
            })
            .collect()
    }

    fn caveat(&self) -> Option<&str> {
        self.caveat.as_deref()
    }
}
