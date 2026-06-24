use std::fs;
use std::path::Path;

use anyhow::{bail, Context, Result};

use crate::harnesses::manifest::validation::{
    validate_binary_name, validate_id, validate_package_name, validate_python_package_name,
};
use crate::harnesses::source::{ensure_alias_available, AddedHarnessSource};
use crate::isolation::spec::IsolationPlan;
use crate::runtimes::registry::RuntimeRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratedPackageKind {
    NpmGlobal,
    NpmIsolated,
    NpxInstaller,
    BunxInstaller,
    PythonTool,
}

impl GeneratedPackageKind {
    pub const fn as_manifest_kind(self) -> &'static str {
        match self {
            Self::NpmGlobal => "npm-global",
            Self::NpmIsolated => "npm-isolated",
            Self::NpxInstaller => "npx-installer",
            Self::BunxInstaller => "bunx-installer",
            Self::PythonTool => "python-tool",
        }
    }
}

pub struct GeneratedHarnessPackage<'a> {
    pub package: &'a str,
    pub alias: &'a str,
    pub runtime: &'a str,
    pub kind: GeneratedPackageKind,
    pub binary: &'a str,
    pub target_isolation: Option<&'a IsolationPlan>,
}

pub fn add_package_harness(
    input: &GeneratedHarnessPackage<'_>,
    data_home: &Path,
    runtimes: &RuntimeRegistry,
) -> Result<AddedHarnessSource> {
    validate_generated_package(input)?;
    ensure_alias_available(input.alias, data_home, runtimes)?;
    let manifest_path = data_home
        .join("hm")
        .join("harnesses.d")
        .join(format!("{}.toml", input.alias));
    if manifest_path.exists() {
        bail!(
            "harness alias '{}' already exists at {}",
            input.alias,
            manifest_path.display()
        );
    }
    write_generated_manifest(input, &manifest_path)?;
    Ok(AddedHarnessSource::new(
        input.alias.to_string(),
        manifest_path,
    ))
}

fn validate_generated_package(input: &GeneratedHarnessPackage<'_>) -> Result<()> {
    validate_id("generated harness alias", input.alias)?;
    validate_binary_name("generated harness", "binary", input.binary)?;
    match input.kind {
        GeneratedPackageKind::PythonTool => {
            validate_python_package_name("generated harness", "package", input.package)
        }
        GeneratedPackageKind::NpmGlobal
        | GeneratedPackageKind::NpmIsolated
        | GeneratedPackageKind::NpxInstaller
        | GeneratedPackageKind::BunxInstaller => {
            validate_package_name("generated harness", "package", input.package)
        }
    }
}

fn write_generated_manifest(
    input: &GeneratedHarnessPackage<'_>,
    manifest_path: &Path,
) -> Result<()> {
    let parent = manifest_path
        .parent()
        .context("generated harness manifest should have a parent")?;
    fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    let temp_path = parent.join(format!(".{}.tmp-{}.toml", input.alias, std::process::id()));
    if temp_path.exists() {
        fs::remove_file(&temp_path)
            .with_context(|| format!("remove stale temp file {}", temp_path.display()))?;
    }
    let manifest = generated_manifest(input);
    let result = fs::write(&temp_path, manifest)
        .with_context(|| format!("write {}", temp_path.display()))
        .and_then(|()| {
            fs::rename(&temp_path, manifest_path).with_context(|| {
                format!(
                    "move generated harness manifest {} to {}",
                    temp_path.display(),
                    manifest_path.display()
                )
            })
        });
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn generated_manifest(input: &GeneratedHarnessPackage<'_>) -> String {
    let (home_subdirs, static_envs) = generated_isolation_fields(input.target_isolation);
    format!(
        "schema_version = 1\nid = \"{}\"\ndisplay_name = \"{}\"\ntarget_runtime = \"{}\"\ndetect_binaries = [\"{}\"]\nlaunch_binary = \"{}\"\n\n[package]\nkind = \"{}\"\npackage = \"{}\"\nself_update = \"managed-by-hm\"\n\n[isolation]\nhome_subdirs = [{}]\nstatic_envs = {{{}}}\n",
        toml_escape(input.alias),
        toml_escape(input.alias),
        toml_escape(input.runtime),
        toml_escape(input.binary),
        toml_escape(input.binary),
        input.kind.as_manifest_kind(),
        toml_escape(input.package),
        home_subdirs,
        static_envs,
    )
}

fn generated_isolation_fields(target_isolation: Option<&IsolationPlan>) -> (String, String) {
    let Some(isolation) = target_isolation else {
        return (String::new(), String::new());
    };
    let home_subdirs = isolation
        .home_subdirs
        .iter()
        .map(|subdir| format!("\"{}\"", toml_escape(subdir)))
        .collect::<Vec<_>>()
        .join(", ");
    let static_envs = isolation
        .static_envs
        .iter()
        .map(|(key, value)| format!(" {} = \"{}\"", key, toml_escape(value)))
        .collect::<Vec<_>>()
        .join(",");
    (home_subdirs, static_envs)
}

fn toml_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
