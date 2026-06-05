pub mod dynamic;

pub use dynamic::RuntimeRegistry;

#[cfg(test)]
#[path = "registry_tests.rs"]
mod dynamic_tests;
