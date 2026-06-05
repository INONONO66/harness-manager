use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use colored::Colorize;

use crate::runtimes::builtin::BUILTIN_RUNTIME_MANIFESTS;
use crate::runtimes::manifest::{parse_toml, RuntimeRecord};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManifestOrigin {
    Builtin,
    User,
}

struct LoadedRuntime {
    record: RuntimeRecord,
    routes: HashSet<String>,
    origin: ManifestOrigin,
    content: String,
}

#[derive(Debug, Clone)]
pub struct RuntimeRegistry {
    records: Vec<RuntimeRecord>,
    routes: Vec<HashSet<String>>,
}

#[derive(Debug, Clone)]
pub struct RuntimeDiscoveryEnv {
    pub xdg_config_home: Option<PathBuf>,
    pub xdg_data_home: Option<PathBuf>,
    pub home: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub enum RuntimeSource {
    Builtins,
    #[cfg(test)]
    Manifest {
        label: String,
        content: String,
    },
    File(PathBuf),
}

impl RuntimeSource {
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

impl RuntimeRegistry {
    pub fn load() -> Result<Self> {
        Self::load_from_env(&RuntimeDiscoveryEnv::from_process())
    }

    #[cfg(test)]
    pub fn builtin_only() -> Result<Self> {
        Self::from_sources(&[RuntimeSource::builtins()])
    }

    pub fn load_from_env(env: &RuntimeDiscoveryEnv) -> Result<Self> {
        let mut sources = vec![RuntimeSource::builtins()];
        sources.extend(discover_config_sources(env)?);
        sources.extend(discover_data_sources(env)?);
        Self::from_sources(&sources)
    }

    pub fn from_sources(sources: &[RuntimeSource]) -> Result<Self> {
        let mut loaded: Vec<LoadedRuntime> = Vec::new();
        for source in sources {
            let origin = source.origin();
            for (label, content) in source.contents()? {
                let record = parse_toml(&label, &content)?;
                let name_route = normalize_runtime_name(&record.name);
                if name_route.is_empty() {
                    anyhow::bail!(
                        "runtime '{}' normalizes to empty route (must contain ASCII alnum)",
                        record.name
                    );
                }
                let mut record_routes: HashSet<String> = HashSet::new();
                record_routes.insert(name_route);
                for binary in &record.binary_names {
                    let binary_route = normalize_runtime_name(binary);
                    if binary_route.is_empty() {
                        anyhow::bail!(
                            "runtime '{}' binary '{}' normalizes to empty route",
                            record.name,
                            binary
                        );
                    }
                    record_routes.insert(binary_route);
                }
                let mut shadowed = Vec::new();
                for (idx, existing) in loaded.iter().enumerate() {
                    let overlap: Option<&String> =
                        record_routes.iter().find(|r| existing.routes.contains(*r));
                    if let Some(route) = overlap {
                        match existing.origin {
                            ManifestOrigin::Builtin => shadowed.push(idx),
                            ManifestOrigin::User => {
                                anyhow::bail!(
                                    "duplicate runtime route '{}' (runtime '{}' collides with '{}')",
                                    route,
                                    record.name,
                                    existing.record.name
                                );
                            }
                        }
                    }
                }
                if shadowed.len() > 1 {
                    let mut names: Vec<String> = shadowed
                        .iter()
                        .map(|idx| loaded[*idx].record.name.clone())
                        .collect();
                    names.sort();
                    anyhow::bail!(
                        "user manifest '{}' would shadow multiple built-in runtimes ({}); \
                         a single user runtime can only replace one builtin",
                        label,
                        names.join(", ")
                    );
                }
                for idx in shadowed.into_iter().rev() {
                    let removed = loaded.swap_remove(idx);
                    record_routes.extend(removed.routes);
                    if removed.content != content {
                        eprintln!(
                            "{} builtin runtime '{}' overridden by user manifest '{}'",
                            "note:".yellow().bold(),
                            removed.record.name,
                            label
                        );
                    }
                }
                loaded.push(LoadedRuntime {
                    record,
                    routes: record_routes,
                    origin,
                    content,
                });
            }
        }
        loaded.sort_by(|left, right| left.record.name.cmp(&right.record.name));
        let (records, routes): (Vec<RuntimeRecord>, Vec<HashSet<String>>) = loaded
            .into_iter()
            .map(|entry| (entry.record, entry.routes))
            .unzip();
        Ok(Self { records, routes })
    }

    pub fn records(&self) -> &[RuntimeRecord] {
        &self.records
    }

    /// Resolve by binary name (exact, case-insensitive) or by normalized display name.
    /// Uses stored routes so a binary-route override still resolves the OLD display name
    /// to the replacement record.
    pub fn find(&self, name: &str) -> Option<&RuntimeRecord> {
        let normalized = normalize_runtime_name(name);
        self.routes
            .iter()
            .position(|r| r.contains(&normalized))
            .map(|idx| &self.records[idx])
    }

    pub fn find_by_display_name(&self, name: &str) -> Option<&RuntimeRecord> {
        self.records.iter().find(|record| record.name == name)
    }

    pub fn id_conflicts_with_runtime(&self, id: &str) -> bool {
        let normalized = normalize_runtime_name(id);
        self.routes.iter().any(|r| r.contains(&normalized))
    }

    pub fn target_runtime_subdir(&self, display_name: &str) -> String {
        self.find_by_display_name(display_name)
            .and_then(|record| record.isolation.as_ref().map(|iso| iso.subdir.clone()))
            .unwrap_or_else(|| normalize_runtime_name(display_name))
    }
}

impl RuntimeDiscoveryEnv {
    pub fn from_process() -> Self {
        Self {
            xdg_config_home: std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from),
            xdg_data_home: std::env::var_os("XDG_DATA_HOME").map(PathBuf::from),
            home: dirs::home_dir(),
        }
    }
}

impl RuntimeSource {
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
            Self::Builtins => Ok(BUILTIN_RUNTIME_MANIFESTS
                .iter()
                .map(|(label, content)| ((*label).to_string(), (*content).to_string()))
                .collect()),
            #[cfg(test)]
            Self::Manifest { label, content } => Ok(vec![(label.clone(), content.clone())]),
            Self::File(path) => {
                let content = fs::read_to_string(path).with_context(|| {
                    format!("failed to read runtime manifest {}", path.display())
                })?;
                Ok(vec![(path.display().to_string(), content)])
            }
        }
    }
}

fn discover_config_sources(env: &RuntimeDiscoveryEnv) -> Result<Vec<RuntimeSource>> {
    if let Some(root) = &env.xdg_config_home {
        return discover_runtime_dir(&root.join("hm").join("runtimes.d"));
    }
    let Some(home) = &env.home else {
        return Ok(Vec::new());
    };
    discover_runtime_dir(&home.join(".config").join("hm").join("runtimes.d"))
}

fn discover_data_sources(env: &RuntimeDiscoveryEnv) -> Result<Vec<RuntimeSource>> {
    let root = env.xdg_data_home.clone().or_else(|| {
        env.home
            .as_ref()
            .map(|home| home.join(".local").join("share"))
    });
    let Some(root) = root else {
        return Ok(Vec::new());
    };

    let mut sources = discover_runtime_dir(&root.join("hm").join("runtimes.d"))?;
    sources.extend(discover_plugin_sources(&root.join("hm").join("plugins"))?);
    Ok(sources)
}

fn discover_runtime_dir(dir: &Path) -> Result<Vec<RuntimeSource>> {
    let mut paths = manifest_files_in_dir(dir)?;
    paths.sort();
    Ok(paths.into_iter().map(RuntimeSource::File).collect())
}

fn discover_plugin_sources(dir: &Path) -> Result<Vec<RuntimeSource>> {
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
                "plugin runtime path must not traverse symlink: {}",
                entry.path().display()
            );
        }
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path().join("runtime.toml");
        if path.exists() {
            reject_symlink_escape(&canonical_root, &path)?;
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths.into_iter().map(RuntimeSource::File).collect())
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
        anyhow::bail!("runtime manifest must not be a symlink: {}", path.display());
    }
    let canonical = path
        .canonicalize()
        .with_context(|| format!("failed to canonicalize {}", path.display()))?;
    if !canonical.starts_with(root) {
        anyhow::bail!(
            "runtime manifest path escapes discovery root: {}",
            path.display()
        );
    }
    Ok(())
}

fn normalize_runtime_name(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
