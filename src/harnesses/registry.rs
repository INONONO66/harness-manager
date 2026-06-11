mod dynamic;

pub use dynamic::HarnessRegistry;
pub use dynamic::{HarnessDiscoveryEnv, HarnessSource};

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
