use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::config::ResolvedProfile;
use crate::runtimes::manifest::InjectionRecord;

mod codex_config;
mod env;
mod json_config;
mod path;
mod provider_config;
mod validation;

pub use codex_config::{apply_codex_config_seed_strategy, validate_codex_config_seed};
pub use env::apply_env_strategy;
pub use provider_config::{
    apply_provider_config_seed_strategy, validate_provider_config_seed, ProviderConfigSeedSource,
};

pub fn apply_injection(
    injection: &InjectionRecord,
    resolved: &ResolvedProfile,
    env: &mut HashMap<String, String>,
    home_dir: &Path,
) -> Result<Option<PathBuf>> {
    match injection {
        InjectionRecord::Env(spec) => {
            apply_env_strategy(spec, resolved, env)?;
            Ok(None)
        }
        InjectionRecord::ProviderConfigSeed(spec) => {
            let path = apply_provider_config_seed_strategy(spec, resolved, env, home_dir)?;
            Ok(Some(path))
        }
        InjectionRecord::CodexConfigSeed(spec) => {
            let path = apply_codex_config_seed_strategy(spec, resolved, env, home_dir)?;
            Ok(Some(path))
        }
    }
}

#[cfg(test)]
mod tests;
