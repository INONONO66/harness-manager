#![allow(dead_code)]

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::harnesses::builtin::BUILTIN_MANIFESTS;
use crate::harnesses::manifest::{parse_toml, ManifestHarnessSpec};

#[derive(Debug, Clone)]
pub struct HarnessRegistry {
    specs: Vec<ManifestHarnessSpec>,
}

#[derive(Debug, Clone)]
pub struct HarnessDiscoveryEnv {
    pub xdg_config_home: Option<PathBuf>,
    pub xdg_data_home: Option<PathBuf>,
    pub home: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub enum HarnessSource {
    Builtins,
    Manifest { label: String, content: String },
    File(PathBuf),
}

impl HarnessSource {
    pub fn builtins() -> Self {
        Self::Builtins
    }

    pub fn manifest(label: impl Into<String>, content: impl Into<String>) -> Self {
        Self::Manifest {
            label: label.into(),
            content: content.into(),
        }
    }
}

impl HarnessRegistry {
    pub fn load() -> Result<Self> {
        Self::load_from_env(&HarnessDiscoveryEnv::from_process())
    }

    pub fn load_from_env(env: &HarnessDiscoveryEnv) -> Result<Self> {
        let mut sources = vec![HarnessSource::builtins()];
        sources.extend(discover_config_sources(env)?);
        sources.extend(discover_data_sources(env)?);
        Self::from_sources(&sources)
    }

    pub fn from_sources(sources: &[HarnessSource]) -> Result<Self> {
        let mut specs = Vec::new();
        let mut seen = HashSet::new();
        for source in sources {
            for (label, content) in source.contents()? {
                let spec = parse_toml(&label, &content)?;
                if !seen.insert(spec.id.clone()) {
                    anyhow::bail!("duplicate harness id '{}'", spec.id);
                }
                specs.push(spec);
            }
        }
        specs.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(Self { specs })
    }

    pub fn specs(&self) -> &[ManifestHarnessSpec] {
        &self.specs
    }

    pub fn find(&self, name: &str) -> Option<&ManifestHarnessSpec> {
        let lower = name.to_lowercase();
        self.specs.iter().find(|spec| spec.id == lower)
    }
}

impl HarnessDiscoveryEnv {
    fn from_process() -> Self {
        Self {
            xdg_config_home: std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
            xdg_data_home: std::env::var_os("XDG_DATA_HOME").map(PathBuf::from),
            home: dirs::home_dir(),
        }
    }
}

impl HarnessSource {
    fn contents(&self) -> Result<Vec<(String, String)>> {
        match self {
            Self::Builtins => Ok(BUILTIN_MANIFESTS
                .iter()
                .map(|(label, content)| ((*label).to_string(), (*content).to_string()))
                .collect()),
            Self::Manifest { label, content } => Ok(vec![(label.clone(), content.clone())]),
            Self::File(path) => {
                let content = fs::read_to_string(path).with_context(|| {
                    format!("failed to read harness manifest {}", path.display())
                })?;
                Ok(vec![(path.display().to_string(), content)])
            }
        }
    }
}

fn discover_config_sources(env: &HarnessDiscoveryEnv) -> Result<Vec<HarnessSource>> {
    if let Some(root) = &env.xdg_config_home {
        return discover_harness_dir(&root.join("hm").join("harnesses.d"));
    }
    if let Some(root) = dirs::config_dir() {
        let sources = discover_harness_dir(&root.join("hm").join("harnesses.d"))?;
        if !sources.is_empty() {
            return Ok(sources);
        }
    }
    let Some(home) = &env.home else {
        return Ok(Vec::new());
    };
    discover_harness_dir(&home.join(".config").join("hm").join("harnesses.d"))
}

fn discover_data_sources(env: &HarnessDiscoveryEnv) -> Result<Vec<HarnessSource>> {
    let root = env
        .xdg_data_home
        .clone()
        .or_else(dirs::data_dir)
        .or_else(|| {
            env.home
                .as_ref()
                .map(|home| home.join(".local").join("share"))
        });
    let Some(root) = root else {
        return Ok(Vec::new());
    };

    let mut sources = discover_harness_dir(&root.join("hm").join("harnesses.d"))?;
    sources.extend(discover_plugin_sources(&root.join("hm").join("plugins"))?);
    Ok(sources)
}

fn discover_harness_dir(dir: &Path) -> Result<Vec<HarnessSource>> {
    let mut paths = manifest_files_in_dir(dir)?;
    paths.sort();
    Ok(paths.into_iter().map(HarnessSource::File).collect())
}

fn discover_plugin_sources(dir: &Path) -> Result<Vec<HarnessSource>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let canonical_root = dir
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", dir.display()))?;
    let mut paths = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        if entry.file_type()?.is_symlink() {
            anyhow::bail!(
                "plugin manifest path must not traverse symlink: {}",
                entry.path().display()
            );
        }
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path().join("harness.toml");
        if path.exists() {
            reject_symlink_escape(&canonical_root, &path)?;
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths.into_iter().map(HarnessSource::File).collect())
}

fn manifest_files_in_dir(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let canonical_root = dir
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", dir.display()))?;
    let mut paths = Vec::new();
    for entry in fs::read_dir(dir).with_context(|| format!("failed to read {}", dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("toml") {
            continue;
        }
        reject_symlink_escape(&canonical_root, &path)?;
        paths.push(path);
    }
    Ok(paths)
}

fn reject_symlink_escape(root: &Path, path: &Path) -> Result<()> {
    if fs::symlink_metadata(path)?.file_type().is_symlink() {
        anyhow::bail!("harness manifest must not be a symlink: {}", path.display());
    }
    let canonical = path
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", path.display()))?;
    if !canonical.starts_with(root) {
        anyhow::bail!(
            "harness manifest path escapes discovery root: {}",
            path.display()
        );
    }
    Ok(())
}
