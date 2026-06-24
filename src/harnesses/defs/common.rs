//! Shared data helpers for harness definitions.
//!
//! Several harnesses target the same runtime and therefore reuse the same
//! isolation building blocks (the Codex `config.toml` seed, the Claude env
//! block, …). These helpers keep that data in one place. They are opt-in DATA
//! builders, not name-based dispatch: a harness calls the helper it needs.

use crate::isolation::spec::SeedFilePlan;

/// `CODEX_HOME` static env shared by every Codex-targeting harness.
pub(super) fn codex_home_env() -> (String, String) {
    ("CODEX_HOME".to_string(), "{home}/.codex".to_string())
}

/// Codex `config.toml` seed shared by every Codex-targeting harness.
/// Byte-identical to the Codex runtime seed (`src/runtimes/defs/codex.rs`).
pub(super) fn codex_config_seed() -> SeedFilePlan {
    SeedFilePlan {
        path: "{home}/.codex/config.toml".to_string(),
        content: "analytics_enabled = false\n\
                  check_for_update_on_startup = false\n\
                  cli_auth_credentials_store = \"file\"\n\
                  mcp_oauth_credentials_store = \"file\"\n"
            .to_string(),
        overwrite: false,
        mode: None,
    }
}

/// Claude env block shared by Claude-targeting harnesses, returned in BTreeMap
/// (sorted-key) order to match the legacy TOML→BTreeMap representation.
pub(super) fn claude_static_envs() -> Vec<(String, String)> {
    vec![
        (
            "CLAUDE_CODE_DEBUG_LOGS_DIR".to_string(),
            "{state}/logs".to_string(),
        ),
        (
            "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC".to_string(),
            "1".to_string(),
        ),
        (
            "CLAUDE_CODE_SUBPROCESS_ENV_SCRUB".to_string(),
            "1".to_string(),
        ),
        ("CLAUDE_CODE_TMPDIR".to_string(), "{tmp}".to_string()),
        (
            "CLAUDE_CONFIG_DIR".to_string(),
            "{home}/.claude".to_string(),
        ),
        ("DISABLE_AUTOUPDATER".to_string(), "1".to_string()),
        ("DISABLE_UPDATES".to_string(), "1".to_string()),
    ]
}

/// Claude `settings.json` seed builder for Claude-targeting harnesses.
pub(super) fn claude_settings_seed(content: &str) -> SeedFilePlan {
    SeedFilePlan {
        path: "{home}/.claude/settings.json".to_string(),
        content: content.to_string(),
        overwrite: false,
        mode: None,
    }
}
