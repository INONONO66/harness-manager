pub mod defs;
pub mod detect;
pub mod install;
mod package;
pub mod registry;
mod spec;
pub mod state;
pub mod types;

pub fn load_registry(
    runtimes: &crate::runtimes::registry::RuntimeRegistry,
) -> anyhow::Result<registry::HarnessRegistry> {
    registry::HarnessRegistry::load(runtimes)
}
