use std::collections::BTreeMap;

use crate::isolation::spec::IsolationPlan;
use crate::runtimes::manifest::{
    AuthLoginRecord, AuthProbeRecord, ConfigLocatorRecord, InjectionRecord,
    ProviderConfigSeedInjection, RuntimeRecord, SharedStatePlan,
};

pub fn record() -> RuntimeRecord {
    RuntimeRecord {
        name: "Pi".to_string(),
        binary_names: vec!["pi".to_string()],
        version_arg: "--version".to_string(),
        config_locator: ConfigLocatorRecord::EnvOrHome {
            env: "PI_CODING_AGENT_DIR".to_string(),
            home_relative: ".pi/agent".to_string(),
        },
        config_files: vec!["settings.json".to_string()],
        auth_probes: vec![AuthProbeRecord::ProviderAuthFile {
            relative_path: "auth.json".to_string(),
            label: "Provider auth".to_string(),
        }],
        auth_login: AuthLoginRecord::Unsupported,
        injection: Some(InjectionRecord::ProviderConfigSeed(
            ProviderConfigSeedInjection {
                config_path: "{home}/.pi/agent/models.json".to_string(),
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
                ],
                provider_api_key_envs: BTreeMap::from([
                    ("anthropic".to_string(), "ANTHROPIC_API_KEY".to_string()),
                    ("openai".to_string(), "OPENAI_API_KEY".to_string()),
                    ("google".to_string(), "GOOGLE_API_KEY".to_string()),
                    ("openrouter".to_string(), "OPENROUTER_API_KEY".to_string()),
                    ("groq".to_string(), "GROQ_API_KEY".to_string()),
                    ("xai".to_string(), "XAI_API_KEY".to_string()),
                    ("deepseek".to_string(), "DEEPSEEK_API_KEY".to_string()),
                    ("mistral".to_string(), "MISTRAL_API_KEY".to_string()),
                    ("azure".to_string(), "AZURE_API_KEY".to_string()),
                ]),
                overwrite: false,
                endpoint_strip_v1: true,
                provider_header_overrides: BTreeMap::from([(
                    "anthropic".to_string(),
                    BTreeMap::from([
                        ("x-api-key".to_string(), "{bearer}".to_string()),
                        ("Authorization".to_string(), "Bearer {bearer}".to_string()),
                    ]),
                )]),
                legacy_provider: None,
            },
        )),
        isolation: Some(IsolationPlan {
            subdir: "pi".to_string(),
            runtime_subdir: "pi".to_string(),
            spoof_home: false,
            home_subdirs: vec![".pi/agent".to_string()],
            static_envs: vec![
                (
                    "PI_CODING_AGENT_DIR".to_string(),
                    "{home}/.pi/agent".to_string(),
                ),
                ("PI_OFFLINE".to_string(), "1".to_string()),
                ("PI_SKIP_VERSION_CHECK".to_string(), "1".to_string()),
                ("PI_TELEMETRY".to_string(), "0".to_string()),
            ],
            seed_files: vec![],
            caveat: None,
        }),
        keychain_isolation: None,
        shared_state: Some(SharedStatePlan {
            database_dirs: vec![],
            session_dirs: vec![".pi/agent/sessions".to_string()],
            session_files: vec![],
            session_dir_globs: vec![],
            session_file_globs: vec![],
            auth_files: vec![],
        }),
    }
}
