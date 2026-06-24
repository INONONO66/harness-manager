mod dynamic;

pub use dynamic::HarnessRegistry;

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
