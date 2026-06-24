use crate::isolation::spec::IsolationPlan;
use crate::runtimes::manifest::{
    AuthLoginRecord, AuthProbeRecord, ConfigLocatorRecord, EnvInjection, InjectionRecord,
    RuntimeRecord, SharedStatePlan,
};

pub fn record() -> RuntimeRecord {
    RuntimeRecord {
        name: "Grok CLI".to_string(),
        binary_names: vec!["grok".to_string()],
        version_arg: "--version".to_string(),
        config_locator: ConfigLocatorRecord::EnvOrHome {
            env: "".to_string(),
            home_relative: ".grok".to_string(),
        },
        config_files: vec!["user-settings.json".to_string()],
        auth_probes: vec![
            AuthProbeRecord::JsonFile {
                relative_path: "user-settings.json".to_string(),
                existence_field: "apiKey".to_string(),
                label: "API key".to_string(),
            },
            AuthProbeRecord::EnvKeys {
                vars: vec!["GROK_API_KEY".to_string()],
                label: "API key".to_string(),
            },
        ],
        auth_login: AuthLoginRecord::Message {
            lines: vec![
                "Grok CLI uses a Grok API key.".to_string(),
                "Set `GROK_API_KEY`, run `grok -k <key>`, or save apiKey in ~/.grok/user-settings.json."
                    .to_string(),
                "Install with: curl -fsSL https://raw.githubusercontent.com/superagent-ai/grok-cli/main/install.sh | bash"
                    .to_string(),
            ],
        },
        injection: Some(InjectionRecord::Env(EnvInjection {
            provider: "xai".to_string(),
            supported_providers: vec!["xai".to_string()],
            endpoint_env: "GROK_BASE_URL".to_string(),
            api_key_env: "GROK_API_KEY".to_string(),
            strip_envs: vec![
                "GROK_API_KEY".to_string(),
                "GROK_BASE_URL".to_string(),
                "GROK_MODEL".to_string(),
                "GROK_MAX_TOKENS".to_string(),
            ],
            endpoint_strip_v1: false,
        })),
        spoof_home: false,
        isolation: Some(IsolationPlan {
            subdir: "grok".to_string(),
            runtime_subdir: "grok".to_string(),
            home_subdirs: vec![".grok".to_string()],
            static_envs: vec![],
            seed_files: vec![],
            caveat: None,
        }),
        keychain_isolation: None,
        shared_state: Some(SharedStatePlan {
            database_dirs: vec![],
            session_dirs: vec![".grok/sessions".to_string()],
            session_files: vec![],
            session_dir_globs: vec![],
            session_file_globs: vec![],
            auth_files: vec![],
        }),
    }
}
