use crate::runtimes::manifest::RuntimeRecord;

pub mod claude;
pub mod codex;
pub mod gajae_code;
pub mod grok;
pub mod opencode;
pub mod pi;

pub fn all() -> Vec<RuntimeRecord> {
    Vec::new()
}
