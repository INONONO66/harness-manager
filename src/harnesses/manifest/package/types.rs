use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageCommandTemplate {
    pub argv: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SelfUpdatePolicy {
    SuppressedByEnv,
    ManagedByHm,
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestPackageSpec {
    NpmGlobal {
        package: String,
        self_update: Option<SelfUpdatePolicy>,
    },
    /// Like NpmGlobal but installs into the harness isolation home via
    /// `NPM_CONFIG_PREFIX`, so the binary never lands on the host PATH.
    NpmIsolated {
        package: String,
        self_update: Option<SelfUpdatePolicy>,
    },
    NpxInstaller {
        package: String,
        args: Vec<String>,
        self_update: Option<SelfUpdatePolicy>,
    },
    BunxInstaller {
        package: String,
        args: Vec<String>,
        self_update: Option<SelfUpdatePolicy>,
    },
    PythonTool {
        package: String,
        self_update: Option<SelfUpdatePolicy>,
    },
    Manual {
        instructions: String,
        self_update: Option<SelfUpdatePolicy>,
    },
    Custom {
        install: PackageCommandTemplate,
        update: Option<PackageCommandTemplate>,
        uninstall: Option<PackageCommandTemplate>,
        bin_subdir: Option<String>,
        self_update: Option<SelfUpdatePolicy>,
    },
    GitWorktree {
        repository: String,
        setup: PackageCommandTemplate,
        update: Option<PackageCommandTemplate>,
        self_update: Option<SelfUpdatePolicy>,
    },
}

impl ManifestPackageSpec {
    pub fn bin_subdir(&self) -> Option<&str> {
        match self {
            Self::NpmIsolated { .. } => Some(".npm/bin"),
            Self::PythonTool { .. } => Some(".local/bin"),
            Self::Custom { bin_subdir, .. } => bin_subdir.as_deref(),
            Self::NpmGlobal { .. }
            | Self::NpxInstaller { .. }
            | Self::BunxInstaller { .. }
            | Self::Manual { .. }
            | Self::GitWorktree { .. } => None,
        }
    }
}
