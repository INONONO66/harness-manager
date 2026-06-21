use std::collections::BTreeMap;

use crate::isolation::spec::IsolationPlan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRecord {
    pub name: String,
    pub binary_names: Vec<String>,
    pub version_arg: String,
    pub config_locator: ConfigLocatorRecord,
    pub config_files: Vec<String>,
    pub auth_probes: Vec<AuthProbeRecord>,
    pub auth_login: AuthLoginRecord,
    pub injection: Option<InjectionRecord>,
    pub isolation: Option<IsolationPlan>,
    pub keychain_isolation: Option<IsolationPlan>,
    pub shared_state: Option<SharedStatePlan>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigLocatorRecord {
    EnvOrHome {
        env: String,
        home_relative: String,
    },
    XdgConfig {
        subdir: String,
        env_override: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthProbeRecord {
    EnvKeys {
        vars: Vec<String>,
        label: String,
    },
    JsonFile {
        relative_path: String,
        existence_field: String,
        label: String,
    },
    OAuthFile {
        relative_path: String,
        token_field: String,
        label: String,
    },
    NestedOAuthFile {
        relative_path: String,
        path: Vec<String>,
        label: String,
    },
    KeychainHeuristic {
        marker_file: String,
        keychain_service: Option<String>,
        label: String,
    },
    DataDirJsonFile {
        data_subdir: String,
        file_name: String,
        label: String,
    },
    ProviderAuthFile {
        relative_path: String,
        label: String,
    },
    CodexAuthFile {
        relative_path: String,
        oauth_label: String,
        api_key_label: String,
        personal_access_token_label: Option<String>,
        agent_identity_label: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthLoginRecord {
    Exec {
        label: String,
        binary: String,
        args: Vec<String>,
    },
    Message {
        lines: Vec<String>,
    },
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InjectionRecord {
    Env(EnvInjection),
    ProviderConfigSeed(ProviderConfigSeedInjection),
    CodexConfigSeed(CodexConfigSeedInjection),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvInjection {
    pub provider: String,
    pub supported_providers: Vec<String>,
    pub endpoint_env: String,
    pub api_key_env: String,
    pub strip_envs: Vec<String>,
    pub endpoint_strip_v1: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderConfigSeedInjection {
    pub config_path: String,
    pub root_key: String,
    pub provider_base_url_key: String,
    pub provider_api_key_key: String,
    pub provider_headers_key: Option<String>,
    pub supported_providers: Vec<String>,
    pub provider_api_key_envs: BTreeMap<String, String>,
    pub overwrite: bool,
    pub endpoint_strip_v1: bool,
    pub provider_header_overrides: BTreeMap<String, BTreeMap<String, String>>,
    pub legacy_provider: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexConfigSeedInjection {
    pub config_path: String,
    pub openai_base_url_key: String,
    pub model_provider_key: String,
    pub model_provider_value: String,
    pub provider: String,
    pub supported_providers: Vec<String>,
    pub api_key_env: String,
    pub strip_envs: Vec<String>,
    pub overwrite: bool,
    pub endpoint_strip_v1: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedStatePlan {
    pub database_dirs: Vec<String>,
    pub session_dirs: Vec<String>,
    pub session_files: Vec<String>,
    pub session_dir_globs: Vec<String>,
    pub session_file_globs: Vec<String>,
    pub auth_files: Vec<String>,
}
