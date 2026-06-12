use anyhow::Result;

use crate::runtimes::manifest::{parse_toml, RuntimeRecord};

include!(concat!(
    env!("OUT_DIR"),
    "/builtin_runtime_manifest_index.rs"
));

#[allow(dead_code)]
pub fn builtin_runtime_records() -> Result<Vec<RuntimeRecord>> {
    BUILTIN_RUNTIME_MANIFESTS
        .iter()
        .map(|(label, content)| parse_toml(label, content))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_runtime_manifests_index_lists_all_expected_manifests() {
        let labels: Vec<&str> = BUILTIN_RUNTIME_MANIFESTS
            .iter()
            .map(|(label, _)| *label)
            .collect();
        assert!(labels.iter().any(|l| l.ends_with("/claude.toml")));
        assert!(labels.iter().any(|l| l.ends_with("/codex.toml")));
        assert!(labels.iter().any(|l| l.ends_with("/gajae-code.toml")));
        assert!(labels.iter().any(|l| l.ends_with("/grok.toml")));
        assert!(labels.iter().any(|l| l.ends_with("/opencode.toml")));
        assert!(labels.iter().any(|l| l.ends_with("/pi.toml")));
        assert_eq!(labels.len(), 6);
    }

    #[test]
    fn builtin_runtime_records_all_parse() {
        let records = builtin_runtime_records().expect("builtins parse");
        assert_eq!(records.len(), 6);
    }
}
