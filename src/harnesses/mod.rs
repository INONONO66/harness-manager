pub mod builtin;
pub mod detect;
pub mod install;
pub mod manifest;
mod package;
pub mod registry;
pub mod types;

pub fn load_registry(
    runtimes: &crate::runtimes::registry::RuntimeRegistry,
) -> anyhow::Result<registry::HarnessRegistry> {
    registry::HarnessRegistry::load(runtimes)
}
