use crate::isolation::spec::{IsolationPlan, SeedFilePlan};
use crate::runtimes::manifest::{
    AuthLoginRecord, AuthProbeRecord, ConfigLocatorRecord, EnvInjection, InjectionRecord,
    RuntimeRecord, SharedStatePlan,
};

/// Claude Code is the ONLY SpoofHome runtime (`spoof_home: true`): it ignores
/// `CLAUDE_CONFIG_DIR` and falls back to `~/.claude` in documented bugs
/// (#55456, #30230), so HOME spoofing is the only reliable isolation backstop.
pub fn record() -> RuntimeRecord {
    RuntimeRecord {
        name: "Claude Code".to_string(),
        binary_names: vec!["claude".to_string()],
        version_arg: "--version".to_string(),
        config_locator: ConfigLocatorRecord::EnvOrHome {
            env: "CLAUDE_CONFIG_DIR".to_string(),
            home_relative: ".claude".to_string(),
        },
        config_files: vec!["settings.json".to_string()],
        auth_probes: vec![
            AuthProbeRecord::NestedOAuthFile {
                relative_path: ".credentials.json".to_string(),
                path: vec!["claudeAiOauth".to_string(), "accessToken".to_string()],
                label: "OAuth".to_string(),
            },
            AuthProbeRecord::KeychainHeuristic {
                marker_file: "settings.json".to_string(),
                keychain_service: Some("Claude Code-credentials".to_string()),
                label: "OAuth (macOS Keychain)".to_string(),
            },
            AuthProbeRecord::EnvKeys {
                vars: vec![
                    "ANTHROPIC_API_KEY".to_string(),
                    "ANTHROPIC_AUTH_TOKEN".to_string(),
                ],
                label: "API key".to_string(),
            },
        ],
        auth_login: AuthLoginRecord::Exec {
            label: "Claude Code".to_string(),
            binary: "claude".to_string(),
            args: Vec::new(),
        },
        injection: Some(InjectionRecord::Env(EnvInjection {
            provider: "anthropic".to_string(),
            supported_providers: vec!["anthropic".to_string()],
            endpoint_env: "ANTHROPIC_BASE_URL".to_string(),
            api_key_env: "ANTHROPIC_API_KEY".to_string(),
            strip_envs: vec![
                "ANTHROPIC_API_KEY".to_string(),
                "ANTHROPIC_AUTH_TOKEN".to_string(),
                "ANTHROPIC_BASE_URL".to_string(),
            ],
            endpoint_strip_v1: true,
        })),
        spoof_home: true,
        isolation: Some(IsolationPlan {
            subdir: "claude".to_string(),
            runtime_subdir: "claude".to_string(),
            home_subdirs: vec![".claude".to_string()],
            static_envs: [
                ("CLAUDE_CONFIG_DIR", "{home}/.claude"),
                ("CLAUDE_CODE_TMPDIR", "{tmp}"),
                ("CLAUDE_CODE_DEBUG_LOGS_DIR", "{runtime_logs}"),
                ("DISABLE_UPDATES", "1"),
                ("DISABLE_AUTOUPDATER", "1"),
                ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1"),
                ("CLAUDE_CODE_DISABLE_OFFICIAL_MARKETPLACE_AUTOINSTALL", "1"),
                ("ENABLE_CLAUDEAI_MCP_SERVERS", "false"),
                ("CLAUDE_CODE_SUBPROCESS_ENV_SCRUB", "1"),
                ("DISABLE_LOGIN_COMMAND", "1"),
            ]
            .into_iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect(),
            seed_files: vec![
                SeedFilePlan {
                    path: "{home}/.claude/settings.json".to_string(),
                    content: r#"{
  "apiKeyHelper": "{state}/apikey.sh",
  "permissions": { "defaultMode": "ask" }
}
"#
                    .to_string(),
                    overwrite: false,
                    mode: None,
                },
                SeedFilePlan {
                    path: "{state}/apikey.sh".to_string(),
                    content: r#"#!/bin/sh
exec hm secret get claude-api-key
"#
                    .to_string(),
                    overwrite: true,
                    mode: Some(0o700),
                },
            ],
            caveat: Some(
                "Claude default mode uses apiKeyHelper and disables /login to avoid macOS \
                 Keychain. Run `printf '%s' '<key>' | hm secret set claude-api-key` before \
                 first use, or pass --allow-keychain for OAuth mode."
                    .to_string(),
            ),
        }),
        keychain_isolation: Some(IsolationPlan {
            subdir: "claude-keychain".to_string(),
            runtime_subdir: "claude-keychain".to_string(),
            home_subdirs: vec![".claude".to_string()],
            // Same as `isolation` static_envs EXCEPT no DISABLE_LOGIN_COMMAND,
            // so `/login` works for OAuth in --allow-keychain mode.
            static_envs: [
                ("CLAUDE_CONFIG_DIR", "{home}/.claude"),
                ("CLAUDE_CODE_TMPDIR", "{tmp}"),
                ("CLAUDE_CODE_DEBUG_LOGS_DIR", "{runtime_logs}"),
                ("DISABLE_UPDATES", "1"),
                ("DISABLE_AUTOUPDATER", "1"),
                ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1"),
                ("CLAUDE_CODE_DISABLE_OFFICIAL_MARKETPLACE_AUTOINSTALL", "1"),
                ("ENABLE_CLAUDEAI_MCP_SERVERS", "false"),
                ("CLAUDE_CODE_SUBPROCESS_ENV_SCRUB", "1"),
            ]
            .into_iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect(),
            seed_files: Vec::new(),
            caveat: Some(
                "Claude --allow-keychain mode permits OAuth and may read/write macOS Keychain \
                 entry 'Claude Code-credentials'. Use only when you explicitly want Claude \
                 subscription login."
                    .to_string(),
            ),
        }),
        shared_state: Some(SharedStatePlan {
            database_dirs: Vec::new(),
            session_dirs: vec![
                ".claude/projects".to_string(),
                ".claude/transcripts".to_string(),
            ],
            session_files: Vec::new(),
            session_dir_globs: Vec::new(),
            session_file_globs: Vec::new(),
            auth_files: Vec::new(),
        }),
    }
}
