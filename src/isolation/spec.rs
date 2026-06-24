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
    fn home_subdirs(&self) -> Vec<&str>;
    fn static_envs(&self) -> Vec<(&str, &str)>;
    fn seed_files(&self) -> Vec<SeedFileView<'_>>;
    fn caveat(&self) -> Option<&str>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsolationPlan {
    pub subdir: String,
    pub runtime_subdir: String,
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

impl IsolationRecipe for IsolationPlan {
    fn subdir(&self) -> &str {
        &self.subdir
    }

    fn runtime_subdir(&self) -> &str {
        &self.runtime_subdir
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
