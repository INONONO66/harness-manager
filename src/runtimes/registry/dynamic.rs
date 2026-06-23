use std::collections::HashSet;

use anyhow::Result;

use crate::runtimes::manifest::RuntimeRecord;

#[derive(Debug, Clone)]
pub struct RuntimeRegistry {
    records: Vec<RuntimeRecord>,
    routes: Vec<HashSet<String>>,
}

impl RuntimeRegistry {
    pub fn load() -> Result<Self> {
        let records = crate::runtimes::defs::all();
        let routes: Vec<HashSet<String>> = records
            .iter()
            .map(|record| {
                let mut set = HashSet::new();
                set.insert(normalize_runtime_name(&record.name));
                for binary in &record.binary_names {
                    set.insert(normalize_runtime_name(binary));
                }
                set
            })
            .collect();
        Ok(Self { records, routes })
    }

    #[cfg(test)]
    pub fn builtin_only() -> Result<Self> {
        Self::load()
    }

    pub fn records(&self) -> &[RuntimeRecord] {
        &self.records
    }

    /// Resolve by binary name (exact, case-insensitive) or by normalized display name.
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
