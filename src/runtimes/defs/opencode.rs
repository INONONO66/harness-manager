use std::collections::BTreeMap;

use crate::isolation::spec::IsolationPlan;
use crate::runtimes::manifest::{
    AuthLoginRecord, AuthProbeRecord, ConfigLocatorRecord, InjectionRecord,
    ProviderConfigSeedInjection, RuntimeRecord, SharedStatePlan,
};

pub fn record() -> RuntimeRecord {
    RuntimeRecord {
        name: "OpenCode".to_string(),
        binary_names: vec!["opencode".to_string()],
        version_arg: "--version".to_string(),
        config_locator: ConfigLocatorRecord::XdgConfig {
            subdir: "opencode".to_string(),
            env_override: "OPENCODE_CONFIG_DIR".to_string(),
        },
        config_files: vec!["opencode.jsonc".to_string(), "opencode.json".to_string()],
        auth_probes: vec![
            AuthProbeRecord::DataDirJsonFile {
                data_subdir: "opencode".to_string(),
                file_name: "auth.json".to_string(),
                label: "Provider auth".to_string(),
            },
            AuthProbeRecord::EnvKeys {
                vars: vec![
                    "ANTHROPIC_API_KEY".to_string(),
                    "OPENAI_API_KEY".to_string(),
                    "GOOGLE_API_KEY".to_string(),
                    "GROQ_API_KEY".to_string(),
                ],
                label: "API key".to_string(),
            },
        ],
        auth_login: AuthLoginRecord::Message {
            lines: vec![
                "OpenCode uses provider-specific authentication.".to_string(),
                "Set API keys via environment variables:".to_string(),
                "  export ANTHROPIC_API_KEY=sk-...".to_string(),
                "  export OPENAI_API_KEY=sk-...".to_string(),
                "Or run `opencode` to authenticate interactively.".to_string(),
            ],
        },
        injection: Some(InjectionRecord::ProviderConfigSeed(
            ProviderConfigSeedInjection {
                config_path: "{home}/.config/opencode/opencode.json".to_string(),
                root_key: "provider".to_string(),
                provider_base_url_key: "options.baseURL".to_string(),
                provider_api_key_key: "options.apiKey".to_string(),
                provider_headers_key: Some("options.headers".to_string()),
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
                    "zhipuai".to_string(),
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
                    ("togetherai".to_string(), "TOGETHER_API_KEY".to_string()),
                    ("fireworks-ai".to_string(), "FIREWORKS_API_KEY".to_string()),
                    ("zai".to_string(), "ZAI_API_KEY".to_string()),
                    ("zhipuai".to_string(), "ZAI_API_KEY".to_string()),
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
        isolation: Some(IsolationPlan {
            subdir: "opencode".to_string(),
            runtime_subdir: "opencode".to_string(),
            spoof_home: false,
            home_subdirs: vec![
                ".config/opencode".to_string(),
                ".local/share/opencode".to_string(),
                ".cache/opencode".to_string(),
                ".local/state/opencode".to_string(),
            ],
            // RedirectOnly (spoof_home=false) inherits the host HOME, so XDG_* MUST
            // NOT be set here: those redirect EVERY XDG-aware host tool (gh, etc.)
            // into the hm tree. OpenCode honors OPENCODE_CONFIG_DIR as its dedicated
            // config override, so we redirect config alone and let data/cache/state
            // fall through to the host's natural XDG paths (= plain `opencode`).
            static_envs: vec![
                (
                    "OPENCODE_CONFIG_DIR".to_string(),
                    "{home}/.config/opencode".to_string(),
                ),
                ("OPENCODE_DISABLE_AUTOUPDATE".to_string(), "1".to_string()),
                ("OPENCODE_DISABLE_PROJECT_CONFIG".to_string(), "1".to_string()),
            ],
            seed_files: vec![],
            caveat: None,
        }),
        keychain_isolation: None,
        shared_state: Some(SharedStatePlan {
            database_dirs: vec![],
            session_dirs: vec![
                ".local/share/opencode/storage/session".to_string(),
                ".local/share/opencode/storage/message".to_string(),
                ".local/share/opencode/storage/part".to_string(),
                ".local/share/opencode/storage/session_diff".to_string(),
            ],
            session_files: vec![],
            session_dir_globs: vec![".local/share/opencode/project/*/storage".to_string()],
            session_file_globs: vec![".local/share/opencode/opencode.db*".to_string()],
            auth_files: vec![],
        }),
    }
}
