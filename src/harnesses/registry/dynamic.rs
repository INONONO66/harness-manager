use std::collections::HashSet;

use anyhow::Result;

use crate::harnesses::defs;
use crate::harnesses::spec::HarnessSpec;
use crate::runtimes::registry::RuntimeRegistry;

#[derive(Debug, Clone)]
pub struct HarnessRegistry {
    specs: Vec<HarnessSpec>,
}

impl HarnessRegistry {
    /// Build the registry from the native harness definitions, resolving each
    /// harness's shared-state policy from its target runtime. Fails closed on a
    /// missing target runtime or any conflicting/duplicate route — these are
    /// programming errors, since the defs are compile-time data.
    pub fn load(runtimes: &RuntimeRegistry) -> Result<Self> {
        Self::from_specs(runtimes, defs::all())
    }

    pub(crate) fn from_specs(
        runtimes: &RuntimeRegistry,
        mut specs: Vec<HarnessSpec>,
    ) -> Result<Self> {
        let mut routes: HashSet<String> = HashSet::new();
        for spec in &mut specs {
            let runtime = runtimes
                .find_by_display_name(&spec.target_runtime)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "harness '{}' targets runtime '{}', which is not registered",
                        spec.id,
                        spec.target_runtime
                    )
                })?;
            spec.target_runtime_shared_state = runtime.shared_state.clone();

            for route in std::iter::once(&spec.id).chain(spec.aliases.iter()) {
                if runtimes.id_conflicts_with_runtime(route) {
                    anyhow::bail!("harness route '{route}' conflicts with a runtime command");
                }
                if !routes.insert(route.to_lowercase()) {
                    anyhow::bail!("duplicate harness route '{route}'");
                }
            }
        }
        specs.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(Self { specs })
    }

    #[cfg(test)]
    pub fn builtin_only(runtimes: &RuntimeRegistry) -> Result<Self> {
        Self::load(runtimes)
    }

    pub fn specs(&self) -> &[HarnessSpec] {
        &self.specs
    }

    pub fn find(&self, name: &str) -> Option<&HarnessSpec> {
        let lower = name.to_lowercase();
        self.specs
            .iter()
            .find(|spec| spec.id == lower || spec.aliases.iter().any(|alias| alias == &lower))
    }
}
