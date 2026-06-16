use anyhow::Result;
use serde::Deserialize;

mod types;

use super::validation::{
    ensure, validate_args, validate_binary_name, validate_package_name,
    validate_python_package_name, validate_relative_path,
};
pub use types::{ManifestPackageSpec, PackageCommandTemplate, SelfUpdatePolicy};

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub(super) enum PackageManifest {
    #[serde(rename = "npm-global")]
    NpmGlobal {
        package: String,
        #[serde(default)]
        self_update: Option<SelfUpdatePolicy>,
    },
    #[serde(rename = "npm-isolated")]
    NpmIsolated {
        package: String,
        #[serde(default)]
        self_update: Option<SelfUpdatePolicy>,
    },
    #[serde(rename = "npx-installer")]
    NpxInstaller {
        package: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        self_update: Option<SelfUpdatePolicy>,
    },
    #[serde(rename = "bunx-installer")]
    BunxInstaller {
        package: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        self_update: Option<SelfUpdatePolicy>,
    },
    #[serde(rename = "python-tool")]
    PythonTool {
        package: String,
        #[serde(default)]
        self_update: Option<SelfUpdatePolicy>,
    },
    #[serde(rename = "manual")]
    Manual {
        instructions: String,
        #[serde(default)]
        self_update: Option<SelfUpdatePolicy>,
    },
    #[serde(rename = "custom")]
    Custom {
        install: Vec<String>,
        #[serde(default)]
        update: Option<Vec<String>>,
        #[serde(default)]
        uninstall: Option<Vec<String>>,
        #[serde(default)]
        bin_subdir: Option<String>,
        #[serde(default)]
        self_update: Option<SelfUpdatePolicy>,
    },
    #[serde(rename = "git-worktree")]
    GitWorktree {
        repository: String,
        setup: Vec<String>,
        #[serde(default)]
        update: Option<Vec<String>>,
        #[serde(default)]
        self_update: Option<SelfUpdatePolicy>,
    },
}

pub(super) fn convert_package(
    path_label: &str,
    package: PackageManifest,
) -> Result<ManifestPackageSpec> {
    Ok(match package {
        PackageManifest::NpmGlobal {
            package,
            self_update,
        } => {
            validate_package_name(path_label, "package.package", &package)?;
            ManifestPackageSpec::NpmGlobal {
                package,
                self_update,
            }
        }
        PackageManifest::NpmIsolated {
            package,
            self_update,
        } => {
            validate_package_name(path_label, "package.package", &package)?;
            ManifestPackageSpec::NpmIsolated {
                package,
                self_update,
            }
        }
        PackageManifest::NpxInstaller {
            package,
            args,
            self_update,
        } => {
            validate_package_name(path_label, "package.package", &package)?;
            validate_args(path_label, "package.args", &args)?;
            ManifestPackageSpec::NpxInstaller {
                package,
                args,
                self_update,
            }
        }
        PackageManifest::BunxInstaller {
            package,
            args,
            self_update,
        } => {
            validate_package_name(path_label, "package.package", &package)?;
            validate_args(path_label, "package.args", &args)?;
            ManifestPackageSpec::BunxInstaller {
                package,
                args,
                self_update,
            }
        }
        PackageManifest::PythonTool {
            package,
            self_update,
        } => {
            validate_python_package_name(path_label, "package.package", &package)?;
            ManifestPackageSpec::PythonTool {
                package,
                self_update,
            }
        }
        PackageManifest::Manual {
            instructions,
            self_update,
        } => {
            ensure(
                !instructions.trim().is_empty(),
                path_label,
                "package.instructions",
            )?;
            ManifestPackageSpec::Manual {
                instructions,
                self_update,
            }
        }
        PackageManifest::Custom {
            install,
            update,
            uninstall,
            bin_subdir,
            self_update,
        } => {
            let install = validate_command_template(path_label, "package.install", install)?;
            let update = update
                .map(|argv| validate_command_template(path_label, "package.update", argv))
                .transpose()?;
            let uninstall = uninstall
                .map(|argv| validate_command_template(path_label, "package.uninstall", argv))
                .transpose()?;
            if let Some(subdir) = &bin_subdir {
                validate_relative_path(path_label, "package.bin_subdir", subdir)?;
            }
            ManifestPackageSpec::Custom {
                install,
                update,
                uninstall,
                bin_subdir,
                self_update,
            }
        }
        PackageManifest::GitWorktree {
            repository,
            setup,
            update,
            self_update,
        } => {
            validate_git_repository(path_label, &repository)?;
            let setup = validate_command_template(path_label, "package.setup", setup)?;
            let update = update
                .map(|argv| validate_command_template(path_label, "package.update", argv))
                .transpose()?;
            ManifestPackageSpec::GitWorktree {
                repository,
                setup,
                update,
                self_update,
            }
        }
    })
}

fn validate_git_repository(path_label: &str, repository: &str) -> Result<()> {
    ensure(!repository.is_empty(), path_label, "package.repository")?;
    ensure(
        repository.starts_with("https://github.com/"),
        path_label,
        "package.repository",
    )?;
    ensure(
        !repository.chars().any(char::is_control),
        path_label,
        "package.repository",
    )?;
    ensure(
        !repository.contains(' ') && !repository.contains(".."),
        path_label,
        "package.repository",
    )
}

fn validate_command_template(
    path_label: &str,
    field: &str,
    argv: Vec<String>,
) -> Result<PackageCommandTemplate> {
    ensure(!argv.is_empty(), path_label, field)?;
    validate_binary_name(path_label, field, &argv[0])?;
    validate_custom_program(path_label, field, &argv[0])?;
    validate_args(path_label, field, &argv[1..])?;
    Ok(PackageCommandTemplate { argv })
}

fn validate_custom_program(path_label: &str, field: &str, program: &str) -> Result<()> {
    ensure(
        !matches!(
            program,
            "sh" | "bash"
                | "zsh"
                | "fish"
                | "dash"
                | "ksh"
                | "csh"
                | "tcsh"
                | "pwsh"
                | "powershell"
                | "cmd"
        ),
        path_label,
        field,
    )
}
