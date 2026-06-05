pub mod builtin;
pub mod detect;
pub mod install;
pub mod manifest;
mod package;
pub mod registry;
pub mod types;

pub fn load_registry() -> anyhow::Result<registry::HarnessRegistry> {
    registry::HarnessRegistry::load()
}
