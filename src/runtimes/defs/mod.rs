use crate::runtimes::manifest::RuntimeRecord;

pub mod claude;
pub mod codex;
pub mod gajae_code;
pub mod grok;
pub mod opencode;
pub mod pi;

pub fn all() -> Vec<RuntimeRecord> {
    vec![
        claude::record(),
        codex::record(),
        opencode::record(),
        pi::record(),
        gajae_code::record(),
        grok::record(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_returns_exactly_6_records() {
        assert_eq!(all().len(), 6);
    }

    #[test]
    fn all_records_have_non_empty_names_and_binaries() {
        for r in all() {
            assert!(!r.name.is_empty(), "empty name");
            assert!(!r.binary_names.is_empty(), "no binary: {}", r.name);
        }
    }

    #[test]
    fn only_claude_has_spoof_home_true() {
        for r in all() {
            let spoof = r.isolation.as_ref().map(|i| i.spoof_home).unwrap_or(false);
            if r.name == "Claude Code" {
                assert!(spoof, "Claude should have spoof_home=true");
            } else {
                assert!(!spoof, "{} should have spoof_home=false", r.name);
            }
        }
    }

    #[test]
    fn opencode_static_envs_has_opencode_config_dir_no_xdg() {
        let opencode = all().into_iter().find(|r| r.name == "OpenCode").unwrap();
        let iso = opencode.isolation.unwrap();
        let keys: Vec<&str> = iso.static_envs.iter().map(|(k, _)| k.as_str()).collect();
        assert!(
            keys.contains(&"OPENCODE_CONFIG_DIR"),
            "missing OPENCODE_CONFIG_DIR"
        );
        let xdg_found: Vec<&str> = keys
            .iter()
            .filter(|k| k.starts_with("XDG_"))
            .cloned()
            .collect();
        assert!(xdg_found.is_empty(), "found XDG_ keys: {:?}", xdg_found);
    }

    #[test]
    fn codex_static_envs_has_only_codex_home() {
        let codex = all().into_iter().find(|r| r.name == "Codex CLI").unwrap();
        let iso = codex.isolation.unwrap();
        assert_eq!(iso.static_envs.len(), 1);
        assert_eq!(iso.static_envs[0].0, "CODEX_HOME");
    }

    #[test]
    fn claude_has_keychain_isolation() {
        let claude = all().into_iter().find(|r| r.name == "Claude Code").unwrap();
        assert!(
            claude.keychain_isolation.is_some(),
            "Claude missing keychain_isolation"
        );
    }
}
