use std::collections::HashSet;

use anyhow::Result;
use colored::Colorize;

use crate::runtimes::manifest::{parse_toml, RuntimeRecord};

mod discovery;
mod source;

pub use discovery::RuntimeDiscoveryEnv;
pub use source::RuntimeSource;

use discovery::{discover_config_sources, discover_data_sources};
use source::{LoadedRuntime, ManifestOrigin};

#[derive(Debug, Clone)]
pub struct RuntimeRegistry {
    records: Vec<RuntimeRecord>,
    routes: Vec<HashSet<String>>,
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
}

fn normalize_runtime_name(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
