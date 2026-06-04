use crate::runtimes::types::IsolationSpec;

/// Declarative spec for one harness (a layer that wraps an existing runtime
/// with a plugin / prompt / workflow set).
///
/// A harness shares its target runtime's binary at launch time, but isolates
/// itself in its own `$HM/runtimes/<subdir>/` tree so its config never bleeds
/// into the base runtime's config.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct HarnessSpec {
    /// CLI identifier — what the user types: "omx", "omc", etc. Lowercase.
    pub id: &'static str,
    /// Human-facing display name: "oh-my-codex".
    pub display_name: &'static str,
    /// Must match a `RuntimeSpec.name` in `runtimes::registry::RUNTIMES`.
    pub target_runtime: &'static str,
    /// How this harness is installed.
    pub package: PackageSpec,
    /// Binaries to check via `which` to determine if the harness is installed.
    pub detect_binaries: &'static [&'static str],
    /// Per-harness isolation recipe. Embedded by value (unlike runtimes which
    /// reference a shared static) because each harness owns its own subdir.
    pub isolation: IsolationSpec,
    /// If `Some`, launch this binary instead of the runtime binary
    /// (e.g. `lazycodex-ai` is a wrapper that spawns codex internally).
    /// `None` means launch the runtime's own binary.
    pub launch_binary: Option<&'static str>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum PackageSpec {
    /// Installed via `npm install -g <package>`.
    NpmGlobal { package: &'static str },
    /// Installed via a Python tool installer (uv tool / pipx / pip --user).
    PythonTool { package: &'static str },
    /// Manual install — show instructions to the user.
    Manual { instructions: &'static str },
}
