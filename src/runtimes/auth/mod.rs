use std::path::Path;

use super::manifest::AuthProbeRecord;
use super::types::AuthStatus;

mod codex;
mod env;
mod json;
mod jwt;
mod keychain;
mod oauth;
mod provider;
mod util;

/// Run all auth probes and collect every match (not first-match-wins).
pub fn probe_auth_all(probes: &[AuthProbeRecord], config_dir: Option<&Path>) -> Vec<AuthStatus> {
    let mut results = Vec::new();
    for probe in probes {
        let result = run_probe(probe, config_dir);
        if !matches!(result, AuthStatus::NotConfigured) {
            results.push(result);
        }
    }
    results
}

fn run_probe(probe: &AuthProbeRecord, config_dir: Option<&Path>) -> AuthStatus {
    match probe {
        AuthProbeRecord::EnvKeys { vars, label } => env::probe_env_keys(vars, label),
        AuthProbeRecord::JsonFile {
            relative_path,
            existence_field,
            label,
        } => json::probe_json_file(config_dir, relative_path, existence_field, label)
            .unwrap_or(AuthStatus::NotConfigured),
        AuthProbeRecord::NestedOAuthFile {
            relative_path,
            path,
            label,
        } => oauth::probe_nested_oauth(config_dir, relative_path, path, label)
            .unwrap_or(AuthStatus::NotConfigured),
        AuthProbeRecord::DataDirJsonFile {
            data_subdir,
            file_name,
            label,
        } => provider::probe_data_dir_json(data_subdir, file_name, label)
            .unwrap_or(AuthStatus::NotConfigured),
        AuthProbeRecord::ProviderAuthFile {
            relative_path,
            label,
        } => provider::probe_provider_auth_file(config_dir, relative_path, label)
            .unwrap_or(AuthStatus::NotConfigured),
        AuthProbeRecord::KeychainHeuristic {
            marker_file,
            keychain_service,
            label,
        } => keychain::probe_keychain(config_dir, marker_file, keychain_service.as_deref(), label),
        AuthProbeRecord::CodexAuthFile {
            relative_path,
            oauth_label,
            api_key_label,
            personal_access_token_label,
            agent_identity_label,
        } => codex::probe_codex_auth_file(
            config_dir,
            relative_path,
            oauth_label,
            api_key_label,
            personal_access_token_label.as_deref(),
            agent_identity_label.as_deref(),
        )
        .unwrap_or(AuthStatus::NotConfigured),
    }
}
