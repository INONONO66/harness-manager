//! Native harness domain types.
//!
//! `HarnessSpec` is the in-memory description of a harness (identity + package
//! strategy + isolation recipe). It used to be produced by parsing a TOML
//! manifest; it is now built directly by the per-harness modules under
//! `crate::harnesses::defs`. `target_runtime_shared_state` is left `None` by the
//! defs and filled by `HarnessRegistry::load` from the target runtime record.

use crate::isolation::spec::IsolationPlan;
use crate::runtimes::manifest::SharedStatePlan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessSpec {
    pub id: String,
    pub aliases: Vec<String>,
    pub display_name: String,
    pub target_runtime: String,
    pub target_runtime_shared_state: Option<SharedStatePlan>,
    pub package: PackageSpec,
    pub detect_binaries: Vec<String>,
    pub launch_binary: Option<String>,
    pub launch_args: Vec<String>,
    pub isolation: IsolationPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SelfUpdatePolicy {
    SuppressedByEnv,
    ManagedByHm,
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageCommandTemplate {
    pub argv: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageSpec {
    // NpmGlobal and Manual are engine-supported strategies (build/detect/install
    // paths and tests) that no current builtin harness constructs.
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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

impl PackageSpec {
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
