use std::collections::BTreeMap;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

use crate::isolation::spec::{IsolationPlan, SeedFilePlan};
use crate::runtimes::manifest::SharedStatePlan;
use crate::runtimes::registry::RuntimeRegistry;

mod validation;

use validation::{
    ensure, parse_mode, validate_args, validate_binary_name, validate_env_key, validate_id,
    validate_package_name, validate_python_package_name, validate_relative_path,
    validate_seed_path, validate_template_value,
};

const MAX_MANIFEST_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestHarnessSpec {
    pub id: String,
    pub aliases: Vec<String>,
    pub display_name: String,
    pub target_runtime: String,
    pub target_runtime_shared_state: Option<SharedStatePlan>,
    pub package: ManifestPackageSpec,
    pub detect_binaries: Vec<String>,
    pub launch_binary: Option<String>,
    pub launch_args: Vec<String>,
    pub isolation: IsolationPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestPackageSpec {
    NpmGlobal {
        package: String,
    },
    /// Like NpmGlobal but installs into the harness isolation home via
    /// `NPM_CONFIG_PREFIX`, so the binary never lands on the host PATH.
    NpmIsolated {
        package: String,
    },
    NpxInstaller {
        package: String,
        args: Vec<String>,
    },
    BunxInstaller {
        package: String,
        args: Vec<String>,
    },
    PythonTool {
        package: String,
    },
    Manual {
        instructions: String,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct HarnessManifest {
    schema_version: u32,
    id: String,
    #[serde(default)]
    aliases: Vec<String>,
    display_name: String,
    target_runtime: String,
    detect_binaries: Vec<String>,
    #[serde(default)]
    launch_binary: Option<String>,
    #[serde(default)]
    launch_args: Vec<String>,
    package: PackageManifest,
    isolation: IsolationManifest,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum PackageManifest {
    #[serde(rename = "npm-global")]
    NpmGlobal { package: String },
    #[serde(rename = "npm-isolated")]
    NpmIsolated { package: String },
    #[serde(rename = "npx-installer")]
    NpxInstaller {
        package: String,
        #[serde(default)]
        args: Vec<String>,
    },
    #[serde(rename = "bunx-installer")]
    BunxInstaller {
        package: String,
        #[serde(default)]
        args: Vec<String>,
    },
    #[serde(rename = "python-tool")]
    PythonTool { package: String },
    #[serde(rename = "manual")]
    Manual { instructions: String },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct IsolationManifest {
    #[serde(default)]
    subdir: Option<String>,
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

pub fn parse_toml(
    path_label: &str,
    input: &str,
    runtimes: &RuntimeRegistry,
) -> Result<ManifestHarnessSpec> {
    if input.len() > MAX_MANIFEST_BYTES {
        bail!("{path_label}: manifest exceeds 64 KiB");
    }

    let manifest: HarnessManifest = toml_edit::de::from_str(input).with_context(|| {
        format!("{path_label}: failed to parse manifest (package.kind or unknown field)")
    })?;
    convert_manifest(path_label, manifest, runtimes)
}

fn convert_manifest(
    path_label: &str,
    manifest: HarnessManifest,
    runtimes: &RuntimeRegistry,
) -> Result<ManifestHarnessSpec> {
    ensure(manifest.schema_version == 1, path_label, "schema_version")?;
    validate_id(path_label, &manifest.id)?;
    ensure(
        !runtimes.id_conflicts_with_runtime(&manifest.id),
        path_label,
        "id",
    )?;
    for alias in &manifest.aliases {
        validate_id(path_label, alias)?;
        ensure(alias != &manifest.id, path_label, "aliases")?;
        ensure(
            !runtimes.id_conflicts_with_runtime(alias),
            path_label,
            "aliases",
        )?;
    }
    let target_runtime_record = runtimes.find(&manifest.target_runtime).ok_or_else(|| {
        anyhow::anyhow!(
            "{}: invalid target_runtime '{}'",
            path_label,
            manifest.target_runtime
        )
    })?;
    let target_runtime = target_runtime_record.name.clone();
    let target_runtime_shared_state = target_runtime_record.shared_state.clone();
    ensure(
        !manifest.detect_binaries.is_empty(),
        path_label,
        "detect_binaries",
    )?;
    for binary in &manifest.detect_binaries {
        validate_binary_name(path_label, "detect_binaries", binary)?;
    }
    if let Some(binary) = &manifest.launch_binary {
        validate_binary_name(path_label, "launch_binary", binary)?;
    }
    validate_args(path_label, "launch_args", &manifest.launch_args)?;

    let package = convert_package(path_label, manifest.package)?;
    let isolation = convert_isolation(path_label, &manifest.id, manifest.isolation)?;

    Ok(ManifestHarnessSpec {
        id: manifest.id,
        aliases: manifest.aliases,
        display_name: manifest.display_name,
        target_runtime,
        target_runtime_shared_state,
        package,
        detect_binaries: manifest.detect_binaries,
        launch_binary: manifest.launch_binary,
        launch_args: manifest.launch_args,
        isolation,
    })
}

fn convert_package(path_label: &str, package: PackageManifest) -> Result<ManifestPackageSpec> {
    Ok(match package {
        PackageManifest::NpmGlobal { package } => {
            validate_package_name(path_label, "package.package", &package)?;
            ManifestPackageSpec::NpmGlobal { package }
        }
        PackageManifest::NpmIsolated { package } => {
            validate_package_name(path_label, "package.package", &package)?;
            ManifestPackageSpec::NpmIsolated { package }
        }
        PackageManifest::NpxInstaller { package, args } => {
            validate_package_name(path_label, "package.package", &package)?;
            validate_args(path_label, "package.args", &args)?;
            ManifestPackageSpec::NpxInstaller { package, args }
        }
        PackageManifest::BunxInstaller { package, args } => {
            validate_package_name(path_label, "package.package", &package)?;
            validate_args(path_label, "package.args", &args)?;
            ManifestPackageSpec::BunxInstaller { package, args }
        }
        PackageManifest::PythonTool { package } => {
            validate_python_package_name(path_label, "package.package", &package)?;
            ManifestPackageSpec::PythonTool { package }
        }
        PackageManifest::Manual { instructions } => {
            ensure(
                !instructions.trim().is_empty(),
                path_label,
                "package.instructions",
            )?;
            ManifestPackageSpec::Manual { instructions }
        }
    })
}

fn convert_isolation(
    path_label: &str,
    id: &str,
    isolation: IsolationManifest,
) -> Result<IsolationPlan> {
    let subdir = isolation.subdir.unwrap_or_else(|| id.to_string());
    let runtime_subdir = subdir.clone();
    validate_relative_path(path_label, "isolation.subdir", &subdir)?;
    for subdir in &isolation.home_subdirs {
        validate_relative_path(path_label, "isolation.home_subdirs", subdir)?;
    }

    let mut static_envs = Vec::with_capacity(isolation.static_envs.len());
    for (key, value) in isolation.static_envs {
        validate_env_key(path_label, &key)?;
        validate_template_value(path_label, "isolation.static_envs", &value)?;
        static_envs.push((key, value));
    }

    let mut seed_files = Vec::with_capacity(isolation.seed_files.len());
    for seed in isolation.seed_files {
        validate_template_value(path_label, "isolation.seed_files", &seed.path)?;
        validate_seed_path(path_label, &seed.path)?;
        seed_files.push(SeedFilePlan {
            path: seed.path,
            content: seed.content,
            overwrite: seed.overwrite,
            mode: parse_mode(path_label, seed.mode.as_deref())?,
        });
    }

    Ok(IsolationPlan {
        subdir,
        runtime_subdir,
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
