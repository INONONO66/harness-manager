use crate::isolation::spec::{IsolationPlan, SeedFilePlan};
use crate::runtimes::manifest::{
    AuthLoginRecord, AuthProbeRecord, CodexConfigSeedInjection, ConfigLocatorRecord,
    InjectionRecord, RuntimeRecord, SharedStatePlan,
};

pub fn record() -> RuntimeRecord {
    RuntimeRecord {
        name: "Codex CLI".to_string(),
        binary_names: vec!["codex".to_string()],
        version_arg: "--version".to_string(),
        config_locator: ConfigLocatorRecord::EnvOrHome {
            env: "CODEX_HOME".to_string(),
            home_relative: ".codex".to_string(),
        },
        config_files: vec!["config.toml".to_string()],
        auth_probes: vec![
            AuthProbeRecord::CodexAuthFile {
                relative_path: "auth.json".to_string(),
                oauth_label: "ChatGPT OAuth".to_string(),
                api_key_label: "API key (OPENAI_API_KEY in auth.json)".to_string(),
                personal_access_token_label: Some("Personal access token".to_string()),
                agent_identity_label: Some("Agent identity".to_string()),
            },
            AuthProbeRecord::EnvKeys {
                vars: vec!["CODEX_API_KEY".to_string(), "OPENAI_API_KEY".to_string()],
                label: "API key".to_string(),
            },
            AuthProbeRecord::EnvKeys {
                vars: vec!["CODEX_ACCESS_TOKEN".to_string()],
                label: "Access token".to_string(),
            },
        ],
        auth_login: AuthLoginRecord::Exec {
            label: "Codex".to_string(),
            binary: "codex".to_string(),
            args: vec!["auth".to_string(), "login".to_string()],
        },
        injection: Some(InjectionRecord::CodexConfigSeed(CodexConfigSeedInjection {
            config_path: "{home}/.codex/config.toml".to_string(),
            openai_base_url_key: "openai_base_url".to_string(),
            model_provider_key: "model_provider".to_string(),
            model_provider_value: "openai".to_string(),
            provider: "openai".to_string(),
            supported_providers: vec!["openai".to_string()],
            api_key_env: "CODEX_API_KEY".to_string(),
            strip_envs: vec![
                "OPENAI_API_KEY".to_string(),
                "OPENAI_BASE_URL".to_string(),
                "CODEX_API_KEY".to_string(),
                "CODEX_ACCESS_TOKEN".to_string(),
            ],
            overwrite: false,
            endpoint_strip_v1: false,
        })),
        isolation: Some(IsolationPlan {
            subdir: "codex".to_string(),
            runtime_subdir: "codex".to_string(),
            spoof_home: false,
            home_subdirs: vec![".codex".to_string()],
            static_envs: vec![("CODEX_HOME".to_string(), "{home}/.codex".to_string())],
            seed_files: vec![SeedFilePlan {
                path: "{home}/.codex/config.toml".to_string(),
                content: "analytics_enabled = false\n\
                          check_for_update_on_startup = false\n\
                          cli_auth_credentials_store = \"file\"\n\
                          mcp_oauth_credentials_store = \"file\"\n"
                    .to_string(),
                overwrite: false,
                mode: None,
            }],
            caveat: None,
        }),
        keychain_isolation: None,
        shared_state: Some(SharedStatePlan {
            database_dirs: vec![],
            session_dirs: vec![
                ".codex/sessions".to_string(),
                ".codex/archived_sessions".to_string(),
            ],
            session_files: vec![".codex/history.jsonl".to_string()],
            session_dir_globs: vec![],
            session_file_globs: vec![],
            auth_files: vec![],
        }),
    }
}
