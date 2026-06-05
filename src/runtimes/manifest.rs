use std::collections::BTreeMap;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::isolation::spec::{IsolationPlan, SeedFilePlan};

mod validation;

use validation::{
    ensure, parse_mode, validate_args, validate_binary_name, validate_config_path,
    validate_display_name, validate_dotted_key, validate_env_name_shape, validate_header_name,
    validate_header_value_template, validate_provider_name, validate_relative_path,
    validate_seed_path, validate_static_env_key, validate_template_value,
};

const MAX_MANIFEST_BYTES: usize = 64 * 1024;

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
    DataDirJsonFile {
        data_subdir: String,
        file_name: String,
        label: String,
    },
    KeychainHeuristic {
        marker_file: String,
        label: String,
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
    pub overwrite: bool,
    pub endpoint_strip_v1: bool,
    pub provider_header_overrides: BTreeMap<String, BTreeMap<String, String>>,
    pub legacy_provider: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RuntimeManifest {
    schema_version: u32,
    name: String,
    binary_names: Vec<String>,
    version_arg: String,
    config_locator: ConfigLocatorManifest,
    #[serde(default)]
    config_files: Vec<String>,
    #[serde(default)]
    auth_probes: Vec<AuthProbeManifest>,
    auth_login: AuthLoginManifest,
    #[serde(default)]
    injection: Option<InjectionManifest>,
    #[serde(default)]
    isolation: Option<IsolationManifest>,
    #[serde(default)]
    keychain_isolation: Option<IsolationManifest>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum ConfigLocatorManifest {
    EnvOrHome {
        #[serde(default)]
        env: String,
        home_relative: String,
    },
    XdgConfig {
        subdir: String,
        #[serde(default)]
        env_override: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum AuthProbeManifest {
    EnvKeys {
        vars: Vec<String>,
        label: String,
    },
    JsonFile {
        relative_path: String,
        #[serde(default)]
        existence_field: String,
        label: String,
    },
    OauthFile {
        relative_path: String,
        token_field: String,
        label: String,
    },
    NestedOauthFile {
        relative_path: String,
        path: Vec<String>,
        label: String,
    },
    DataDirJsonFile {
        data_subdir: String,
        file_name: String,
        label: String,
    },
    KeychainHeuristic {
        marker_file: String,
        label: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
enum AuthLoginManifest {
    Exec {
        label: String,
        binary: String,
        #[serde(default)]
        args: Vec<String>,
    },
    Message {
        lines: Vec<String>,
    },
    Unsupported,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "strategy", rename_all = "kebab-case", deny_unknown_fields)]
enum InjectionManifest {
    Env {
        provider: String,
        #[serde(default)]
        supported_providers: Vec<String>,
        endpoint_env: String,
        api_key_env: String,
        #[serde(default)]
        strip_envs: Vec<String>,
        #[serde(default)]
        endpoint_strip_v1: bool,
    },
    ProviderConfigSeed {
        config_path: String,
        root_key: String,
        provider_base_url_key: String,
        provider_api_key_key: String,
        #[serde(default)]
        provider_headers_key: Option<String>,
        supported_providers: Vec<String>,
        #[serde(default)]
        overwrite: bool,
        #[serde(default)]
        endpoint_strip_v1: bool,
        #[serde(default)]
        provider_header_overrides: BTreeMap<String, BTreeMap<String, String>>,
        #[serde(default)]
        legacy_provider: Option<String>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IsolationManifest {
    subdir: String,
    spoof_home: bool,
    #[serde(default)]
    home_subdirs: Vec<String>,
    #[serde(default)]
    static_envs: BTreeMap<String, String>,
    #[serde(default)]
    seed_files: Vec<SeedFileManifest>,
    #[serde(default)]
    caveat: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SeedFileManifest {
    path: String,
    content: String,
    overwrite: bool,
    #[serde(default)]
    mode: Option<String>,
}

pub fn parse_toml(path_label: &str, input: &str) -> Result<RuntimeRecord> {
    if input.len() > MAX_MANIFEST_BYTES {
        bail!("{path_label}: runtime manifest exceeds 64 KiB");
    }
    let manifest: RuntimeManifest = toml_edit::de::from_str(input)
        .with_context(|| format!("{path_label}: failed to parse runtime manifest"))?;
    convert_manifest(path_label, manifest)
}

fn convert_manifest(path_label: &str, manifest: RuntimeManifest) -> Result<RuntimeRecord> {
    ensure(manifest.schema_version == 1, path_label, "schema_version")?;
    validate_display_name(path_label, &manifest.name)?;
    ensure(
        !manifest.binary_names.is_empty(),
        path_label,
        "binary_names",
    )?;
    for binary in &manifest.binary_names {
        validate_binary_name(path_label, "binary_names", binary)?;
    }
    ensure(!manifest.version_arg.is_empty(), path_label, "version_arg")?;
    for file in &manifest.config_files {
        ensure(!file.is_empty(), path_label, "config_files")?;
        ensure(
            !file.contains('/') && !file.contains('\\'),
            path_label,
            "config_files",
        )?;
    }
    let config_locator = convert_config_locator(path_label, manifest.config_locator)?;
    let auth_probes = manifest
        .auth_probes
        .into_iter()
        .map(|probe| convert_auth_probe(path_label, probe))
        .collect::<Result<Vec<_>>>()?;
    let auth_login = convert_auth_login(path_label, manifest.auth_login)?;
    let injection = manifest
        .injection
        .map(|inj| convert_injection(path_label, inj))
        .transpose()?;
    let isolation = manifest
        .isolation
        .map(|iso| convert_isolation(path_label, "isolation", iso))
        .transpose()?;
    let keychain_isolation = manifest
        .keychain_isolation
        .map(|iso| convert_isolation(path_label, "keychain_isolation", iso))
        .transpose()?;

    Ok(RuntimeRecord {
        name: manifest.name,
        binary_names: manifest.binary_names,
        version_arg: manifest.version_arg,
        config_locator,
        config_files: manifest.config_files,
        auth_probes,
        auth_login,
        injection,
        isolation,
        keychain_isolation,
    })
}

fn convert_config_locator(
    path_label: &str,
    locator: ConfigLocatorManifest,
) -> Result<ConfigLocatorRecord> {
    match locator {
        ConfigLocatorManifest::EnvOrHome { env, home_relative } => {
            if !env.is_empty() {
                validate_env_name_shape(path_label, "config_locator.env", &env)?;
            }
            validate_relative_path(path_label, "config_locator.home_relative", &home_relative)?;
            Ok(ConfigLocatorRecord::EnvOrHome { env, home_relative })
        }
        ConfigLocatorManifest::XdgConfig {
            subdir,
            env_override,
        } => {
            validate_relative_path(path_label, "config_locator.subdir", &subdir)?;
            if !env_override.is_empty() {
                validate_env_name_shape(path_label, "config_locator.env_override", &env_override)?;
            }
            Ok(ConfigLocatorRecord::XdgConfig {
                subdir,
                env_override,
            })
        }
    }
}

fn convert_auth_probe(path_label: &str, probe: AuthProbeManifest) -> Result<AuthProbeRecord> {
    match probe {
        AuthProbeManifest::EnvKeys { vars, label } => {
            ensure(!vars.is_empty(), path_label, "auth_probes.vars")?;
            for var in &vars {
                validate_env_name_shape(path_label, "auth_probes.vars", var)?;
            }
            ensure(!label.is_empty(), path_label, "auth_probes.label")?;
            Ok(AuthProbeRecord::EnvKeys { vars, label })
        }
        AuthProbeManifest::JsonFile {
            relative_path,
            existence_field,
            label,
        } => {
            validate_relative_path(path_label, "auth_probes.relative_path", &relative_path)?;
            ensure(!label.is_empty(), path_label, "auth_probes.label")?;
            Ok(AuthProbeRecord::JsonFile {
                relative_path,
                existence_field,
                label,
            })
        }
        AuthProbeManifest::OauthFile {
            relative_path,
            token_field,
            label,
        } => {
            validate_relative_path(path_label, "auth_probes.relative_path", &relative_path)?;
            ensure(
                !token_field.is_empty(),
                path_label,
                "auth_probes.token_field",
            )?;
            ensure(!label.is_empty(), path_label, "auth_probes.label")?;
            Ok(AuthProbeRecord::OAuthFile {
                relative_path,
                token_field,
                label,
            })
        }
        AuthProbeManifest::NestedOauthFile {
            relative_path,
            path,
            label,
        } => {
            validate_relative_path(path_label, "auth_probes.relative_path", &relative_path)?;
            ensure(!path.is_empty(), path_label, "auth_probes.path")?;
            for segment in &path {
                ensure(!segment.is_empty(), path_label, "auth_probes.path")?;
            }
            ensure(!label.is_empty(), path_label, "auth_probes.label")?;
            Ok(AuthProbeRecord::NestedOAuthFile {
                relative_path,
                path,
                label,
            })
        }
        AuthProbeManifest::DataDirJsonFile {
            data_subdir,
            file_name,
            label,
        } => {
            validate_relative_path(path_label, "auth_probes.data_subdir", &data_subdir)?;
            ensure(!file_name.is_empty(), path_label, "auth_probes.file_name")?;
            ensure(
                !file_name.contains('/') && !file_name.contains('\\'),
                path_label,
                "auth_probes.file_name",
            )?;
            ensure(!label.is_empty(), path_label, "auth_probes.label")?;
            Ok(AuthProbeRecord::DataDirJsonFile {
                data_subdir,
                file_name,
                label,
            })
        }
        AuthProbeManifest::KeychainHeuristic { marker_file, label } => {
            validate_relative_path(path_label, "auth_probes.marker_file", &marker_file)?;
            ensure(!label.is_empty(), path_label, "auth_probes.label")?;
            Ok(AuthProbeRecord::KeychainHeuristic { marker_file, label })
        }
    }
}

fn convert_auth_login(path_label: &str, login: AuthLoginManifest) -> Result<AuthLoginRecord> {
    match login {
        AuthLoginManifest::Exec {
            label,
            binary,
            args,
        } => {
            ensure(!label.is_empty(), path_label, "auth_login.label")?;
            validate_binary_name(path_label, "auth_login.binary", &binary)?;
            validate_args(path_label, "auth_login.args", &args)?;
            Ok(AuthLoginRecord::Exec {
                label,
                binary,
                args,
            })
        }
        AuthLoginManifest::Message { lines } => {
            ensure(!lines.is_empty(), path_label, "auth_login.lines")?;
            Ok(AuthLoginRecord::Message { lines })
        }
        AuthLoginManifest::Unsupported => Ok(AuthLoginRecord::Unsupported),
    }
}

fn convert_injection(path_label: &str, injection: InjectionManifest) -> Result<InjectionRecord> {
    match injection {
        InjectionManifest::Env {
            provider,
            supported_providers,
            endpoint_env,
            api_key_env,
            strip_envs,
            endpoint_strip_v1,
        } => {
            validate_provider_name(path_label, "injection.provider", &provider)?;
            validate_env_name_shape(path_label, "injection.endpoint_env", &endpoint_env)?;
            validate_env_name_shape(path_label, "injection.api_key_env", &api_key_env)?;
            for var in &strip_envs {
                validate_env_name_shape(path_label, "injection.strip_envs", var)?;
            }
            let supported = if supported_providers.is_empty() {
                vec![provider.clone()]
            } else {
                for p in &supported_providers {
                    validate_provider_name(path_label, "injection.supported_providers", p)?;
                }
                ensure(
                    supported_providers.contains(&provider),
                    path_label,
                    "injection.supported_providers",
                )?;
                supported_providers
            };
            Ok(InjectionRecord::Env(EnvInjection {
                provider,
                supported_providers: supported,
                endpoint_env,
                api_key_env,
                strip_envs,
                endpoint_strip_v1,
            }))
        }
        InjectionManifest::ProviderConfigSeed {
            config_path,
            root_key,
            provider_base_url_key,
            provider_api_key_key,
            provider_headers_key,
            supported_providers,
            overwrite,
            endpoint_strip_v1,
            provider_header_overrides,
            legacy_provider,
        } => {
            validate_config_path(path_label, "injection.config_path", &config_path)?;
            validate_template_value(path_label, "injection.config_path", &config_path)?;
            validate_dotted_key(path_label, "injection.root_key", &root_key)?;
            validate_dotted_key(
                path_label,
                "injection.provider_base_url_key",
                &provider_base_url_key,
            )?;
            validate_dotted_key(
                path_label,
                "injection.provider_api_key_key",
                &provider_api_key_key,
            )?;
            if let Some(ref headers_key) = provider_headers_key {
                validate_dotted_key(path_label, "injection.provider_headers_key", headers_key)?;
            }
            ensure(
                !supported_providers.is_empty(),
                path_label,
                "injection.supported_providers",
            )?;
            for p in &supported_providers {
                validate_provider_name(path_label, "injection.supported_providers", p)?;
            }
            for (provider, headers) in &provider_header_overrides {
                validate_provider_name(
                    path_label,
                    "injection.provider_header_overrides",
                    provider,
                )?;
                ensure(
                    supported_providers.contains(provider),
                    path_label,
                    "injection.provider_header_overrides",
                )?;
                for (name, value) in headers {
                    validate_header_name(
                        path_label,
                        "injection.provider_header_overrides.headers",
                        name,
                    )?;
                    validate_header_value_template(
                        path_label,
                        "injection.provider_header_overrides.header_values",
                        value,
                    )?;
                }
            }
            if let Some(ref legacy) = legacy_provider {
                validate_provider_name(path_label, "injection.legacy_provider", legacy)?;
                ensure(
                    supported_providers.contains(legacy),
                    path_label,
                    "injection.legacy_provider",
                )?;
            }
            Ok(InjectionRecord::ProviderConfigSeed(
                ProviderConfigSeedInjection {
                    config_path,
                    root_key,
                    provider_base_url_key,
                    provider_api_key_key,
                    provider_headers_key,
                    supported_providers,
                    overwrite,
                    endpoint_strip_v1,
                    provider_header_overrides,
                    legacy_provider,
                },
            ))
        }
    }
}

fn convert_isolation(
    path_label: &str,
    section_label: &str,
    isolation: IsolationManifest,
) -> Result<IsolationPlan> {
    validate_relative_path(
        path_label,
        &format!("{section_label}.subdir"),
        &isolation.subdir,
    )?;
    for subdir in &isolation.home_subdirs {
        validate_relative_path(path_label, &format!("{section_label}.home_subdirs"), subdir)?;
    }

    let mut static_envs = Vec::with_capacity(isolation.static_envs.len());
    for (key, value) in isolation.static_envs {
        validate_static_env_key(path_label, &key)?;
        validate_template_value(path_label, &format!("{section_label}.static_envs"), &value)?;
        static_envs.push((key, value));
    }

    let mut seed_files = Vec::with_capacity(isolation.seed_files.len());
    for seed in isolation.seed_files {
        validate_template_value(
            path_label,
            &format!("{section_label}.seed_files"),
            &seed.path,
        )?;
        validate_seed_path(path_label, &seed.path)?;
        seed_files.push(SeedFilePlan {
            path: seed.path,
            content: seed.content,
            overwrite: seed.overwrite,
            mode: parse_mode(path_label, seed.mode.as_deref())?,
        });
    }

    Ok(IsolationPlan {
        subdir: isolation.subdir.clone(),
        runtime_subdir: isolation.subdir,
        spoof_home: isolation.spoof_home,
        home_subdirs: isolation.home_subdirs,
        static_envs,
        seed_files,
        caveat: isolation.caveat,
    })
}

#[cfg(test)]
#[path = "manifest_tests.rs"]
mod tests;
