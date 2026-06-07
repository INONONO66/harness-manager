pub mod injection;

mod assembly;
mod profile;
mod runner;
mod target;

use std::collections::HashMap;

use crate::config::HmConfig;
use crate::harnesses::registry::HarnessRegistry;
use crate::runtimes::registry::RuntimeRegistry;

pub type UseEnvAssembly = assembly::UseEnvAssembly;

pub fn effective_profile_name(arg: Option<&str>, hm_config: &HmConfig) -> Option<String> {
    arg.map(str::to_string)
        .or_else(|| hm_config.default_profile.clone())
}

pub fn assemble_use_env(
    runtimes: &RuntimeRegistry,
    harnesses: &HarnessRegistry,
    target_name: &str,
    profile_name: Option<&str>,
    allow_keychain: bool,
    inherited: HashMap<String, String>,
) -> anyhow::Result<UseEnvAssembly> {
    assembly::assemble_use_env(
        runtimes,
        harnesses,
        target_name,
        profile_name,
        allow_keychain,
        inherited,
    )
}

pub fn run_use(
    runtimes: &RuntimeRegistry,
    harnesses: &HarnessRegistry,
    target_name: &str,
    profile_name: Option<&str>,
    print_env: bool,
    allow_keychain: bool,
    extra_args: &[String],
) -> anyhow::Result<()> {
    runner::run_use(
        runtimes,
        harnesses,
        target_name,
        profile_name,
        print_env,
        allow_keychain,
        extra_args,
    )
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
