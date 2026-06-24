use std::collections::BTreeMap;

use crate::isolation::spec::IsolationPlan;
use crate::runtimes::manifest::{
    AuthLoginRecord, AuthProbeRecord, ConfigLocatorRecord, InjectionRecord,
    ProviderConfigSeedInjection, RuntimeRecord, SharedStatePlan,
};

pub fn record() -> RuntimeRecord {
    RuntimeRecord {
        name: "Gajae-Code".to_string(),
        binary_names: vec!["gjc".to_string()],
        version_arg: "--version".to_string(),
        config_locator: ConfigLocatorRecord::EnvOrHome {
            env: "GJC_CODING_AGENT_DIR".to_string(),
            home_relative: ".gjc/agent".to_string(),
        },
        config_files: vec!["config.yml".to_string(), "models.yml".to_string()],
        auth_probes: vec![
            AuthProbeRecord::EnvKeys {
                vars: vec![
                    "ANTHROPIC_API_KEY".to_string(),
                    "ANTHROPIC_OAUTH_TOKEN".to_string(),
                    "OPENAI_API_KEY".to_string(),
                    "GEMINI_API_KEY".to_string(),
                    "GOOGLE_API_KEY".to_string(),
                    "XAI_API_KEY".to_string(),
                    "OPENROUTER_API_KEY".to_string(),
                    "GROQ_API_KEY".to_string(),
                ],
                label: "Provider API key".to_string(),
            },
            AuthProbeRecord::EnvKeys {
                vars: vec![
                    "GJC_AUTH_BROKER_URL".to_string(),
                    "GJC_AUTH_BROKER_TOKEN".to_string(),
                ],
                label: "Auth broker".to_string(),
            },
        ],
        auth_login: AuthLoginRecord::Message {
            lines: vec![
                "Gajae-Code uses provider-specific credentials.".to_string(),
                "Set API keys in the environment, configure ~/.gjc/agent/models.yml, or run `gjc setup --provider <id>`."
                    .to_string(),
                "Install with: bun install -g gajae-code".to_string(),
            ],
        },
        injection: Some(InjectionRecord::ProviderConfigSeed(
            ProviderConfigSeedInjection {
                config_path: "{home}/.gjc/agent/models.yml".to_string(),
                root_key: "providers".to_string(),
                provider_base_url_key: "baseUrl".to_string(),
                provider_api_key_key: "apiKey".to_string(),
                provider_headers_key: Some("headers".to_string()),
                supported_providers: vec![
                    "anthropic".to_string(),
                    "openai".to_string(),
                    "google".to_string(),
                    "openrouter".to_string(),
                    "groq".to_string(),
                    "xai".to_string(),
                    "deepseek".to_string(),
                    "mistral".to_string(),
                    "azure".to_string(),
                    "togetherai".to_string(),
                    "fireworks-ai".to_string(),
                    "zai".to_string(),
                    "cerebras".to_string(),
                ],
                provider_api_key_envs: BTreeMap::from([
                    ("anthropic".to_string(), "ANTHROPIC_API_KEY".to_string()),
                    ("openai".to_string(), "OPENAI_API_KEY".to_string()),
                    ("google".to_string(), "GEMINI_API_KEY".to_string()),
                    ("openrouter".to_string(), "OPENROUTER_API_KEY".to_string()),
                    ("groq".to_string(), "GROQ_API_KEY".to_string()),
                    ("xai".to_string(), "XAI_API_KEY".to_string()),
                    ("deepseek".to_string(), "DEEPSEEK_API_KEY".to_string()),
                    ("mistral".to_string(), "MISTRAL_API_KEY".to_string()),
                    ("azure".to_string(), "AZURE_API_KEY".to_string()),
                    ("togetherai".to_string(), "TOGETHER_API_KEY".to_string()),
                    ("fireworks-ai".to_string(), "FIREWORKS_API_KEY".to_string()),
                    ("zai".to_string(), "ZAI_API_KEY".to_string()),
                    ("cerebras".to_string(), "CEREBRAS_API_KEY".to_string()),
                ]),
                overwrite: false,
                endpoint_strip_v1: false,
                provider_header_overrides: BTreeMap::from([(
                    "anthropic".to_string(),
                    BTreeMap::from([
                        ("x-api-key".to_string(), "{bearer}".to_string()),
                        ("Authorization".to_string(), "Bearer {bearer}".to_string()),
                    ]),
                )]),
                legacy_provider: Some("openai".to_string()),
            },
        )),
        spoof_home: false,
        isolation: Some(IsolationPlan {
            subdir: "gajae-code".to_string(),
            runtime_subdir: "gajae-code".to_string(),
            home_subdirs: vec![".gjc".to_string(), ".cache/gajae-code".to_string()],
            static_envs: vec![
                ("GJC_CODING_AGENT_DIR".to_string(), "{home}/.gjc/agent".to_string()),
                ("GJC_CONFIG_DIR".to_string(), ".gjc".to_string()),
                ("GJC_NO_PTY".to_string(), "1".to_string()),
            ],
            seed_files: vec![],
            caveat: None,
        }),
        keychain_isolation: None,
        shared_state: Some(SharedStatePlan {
            database_dirs: vec![],
            session_dirs: vec![".gjc/agent/sessions".to_string()],
            session_files: vec![],
            session_dir_globs: vec![],
            session_file_globs: vec![".gjc/agent/*.db*".to_string()],
            auth_files: vec![],
        }),
    }
}
