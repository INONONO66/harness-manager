use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::harnesses::builtin::BUILTIN_MANIFESTS;
use crate::harnesses::manifest::{parse_toml, ManifestHarnessSpec};
use crate::runtimes::registry::RuntimeRegistry;

const MAX_MANIFEST_BYTES: u64 = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManifestOrigin {
    Builtin,
    User,
}

struct LoadedHarness {
    spec: ManifestHarnessSpec,
    routes: HashSet<String>,
    origin: ManifestOrigin,
}

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
    #[cfg(test)]
    Manifest {
        label: String,
        content: String,
    },
    File(PathBuf),
}

impl HarnessSource {
    pub fn builtins() -> Self {
        Self::Builtins
    }

    #[cfg(test)]
    pub fn manifest(label: impl Into<String>, content: impl Into<String>) -> Self {
        Self::Manifest {
            label: label.into(),
            content: content.into(),
        }
    }
}

impl HarnessRegistry {
    pub fn load(runtimes: &RuntimeRegistry) -> Result<Self> {
        Self::load_from_env(&HarnessDiscoveryEnv::from_process(), runtimes)
    }

    #[cfg(test)]
    pub fn builtin_only(runtimes: &RuntimeRegistry) -> Result<Self> {
        Self::from_sources(&[HarnessSource::builtins()], runtimes)
    }

    pub fn load_from_env(env: &HarnessDiscoveryEnv, runtimes: &RuntimeRegistry) -> Result<Self> {
        let mut sources = vec![HarnessSource::builtins()];
        sources.extend(discover_config_sources(env)?);
        sources.extend(discover_data_sources(env)?);
        Self::from_sources(&sources, runtimes)
    }

    pub fn from_sources(sources: &[HarnessSource], runtimes: &RuntimeRegistry) -> Result<Self> {
        let mut loaded: Vec<LoadedHarness> = Vec::new();
        for source in sources {
            let origin = source.origin();
            for (label, content) in source.contents()? {
                let spec = parse_toml(&label, &content, runtimes)?;
                let routes: HashSet<String> = std::iter::once(spec.id.clone())
                    .chain(spec.aliases.iter().cloned())
                    .collect();
                let mut shadowed = Vec::new();
                for (idx, existing) in loaded.iter().enumerate() {
                    let overlap: Option<&String> =
                        routes.iter().find(|r| existing.routes.contains(*r));
                    if let Some(route) = overlap {
                        match existing.origin {
                            ManifestOrigin::Builtin => shadowed.push(idx),
                            ManifestOrigin::User => {
                                anyhow::bail!(
                                    "duplicate harness route '{}' (harness '{}' collides with '{}')",
                                    route,
                                    spec.id,
                                    existing.spec.id
                                );
                            }
                        }
                    }
                }
                if shadowed.len() > 1 {
                    let mut ids: Vec<String> = shadowed
                        .iter()
                        .map(|idx| loaded[*idx].spec.id.clone())
                        .collect();
                    ids.sort();
                    anyhow::bail!(
                        "user manifest '{}' would shadow multiple built-in harnesses ({}); \
                         a single user harness can only replace one builtin",
                        label,
                        ids.join(", ")
                    );
                }
                for idx in shadowed.into_iter().rev() {
                    loaded.swap_remove(idx);
                }
                loaded.push(LoadedHarness {
                    spec,
                    routes,
                    origin,
                });
            }
        }
        let mut specs: Vec<ManifestHarnessSpec> = loaded.into_iter().map(|l| l.spec).collect();
        specs.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(Self { specs })
    }

    pub fn specs(&self) -> &[ManifestHarnessSpec] {
        &self.specs
    }

    pub fn find(&self, name: &str) -> Option<&ManifestHarnessSpec> {
        let lower = name.to_lowercase();
        self.specs
            .iter()
            .find(|spec| spec.id == lower || spec.aliases.iter().any(|alias| alias == &lower))
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
    fn origin(&self) -> ManifestOrigin {
        match self {
            Self::Builtins => ManifestOrigin::Builtin,
            #[cfg(test)]
            Self::Manifest { .. } => ManifestOrigin::User,
            Self::File(_) => ManifestOrigin::User,
        }
    }

    fn contents(&self) -> Result<Vec<(String, String)>> {
        match self {
            Self::Builtins => Ok(BUILTIN_MANIFESTS
                .iter()
                .map(|(label, content)| ((*label).to_string(), (*content).to_string()))
                .collect()),
            #[cfg(test)]
            Self::Manifest { label, content } => Ok(vec![(label.clone(), content.clone())]),
            Self::File(path) => {
                ensure_manifest_size(path)?;
                let content = fs::read_to_string(path).with_context(|| {
                    format!("failed to read harness manifest {}", path.display())
                })?;
                Ok(vec![(path.display().to_string(), content)])
            }
        }
    }
}

fn ensure_manifest_size(path: &Path) -> Result<()> {
    let metadata = fs::metadata(path)
        .with_context(|| format!("failed to stat harness manifest {}", path.display()))?;
    if metadata.len() > MAX_MANIFEST_BYTES {
        anyhow::bail!("{}: harness manifest exceeds 64 KiB", path.display());
    }
    Ok(())
}

fn discover_config_sources(env: &HarnessDiscoveryEnv) -> Result<Vec<HarnessSource>> {
    if let Some(root) = &env.xdg_config_home {
        return discover_harness_dir(&root.join("hm").join("harnesses.d"));
    }
    let Some(home) = &env.home else {
        return Ok(Vec::new());
    };
    discover_harness_dir(&home.join(".config").join("hm").join("harnesses.d"))
}

fn discover_data_sources(env: &HarnessDiscoveryEnv) -> Result<Vec<HarnessSource>> {
    let root = env.xdg_data_home.clone().or_else(|| {
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
