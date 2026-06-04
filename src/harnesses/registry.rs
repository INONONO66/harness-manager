use super::types::{HarnessSpec, PackageSpec};
use crate::runtimes::types::{IsolationSpec, SeedFile};

// ---------------------------------------------------------------------------
// Shared seed content
// ---------------------------------------------------------------------------

/// Same minimal codex config used by the base Codex isolation. Reused by
/// every harness that wraps Codex CLI so all three see a consistent setup.
const CODEX_BASE_CONFIG_TOML: &str = concat!(
    "analytics_enabled = false\n",
    "check_for_update_on_startup = false\n",
    "cli_auth_credentials_store = \"file\"\n",
    "mcp_oauth_credentials_store = \"file\"\n",
);

const OMC_CLAUDE_SETTINGS_JSON: &str = concat!(
    "{\n",
    "  \"permissions\": { \"defaultMode\": \"ask\" },\n",
    "  \"enabledPlugins\": { \"oh-my-claudecode\": true }\n",
    "}\n",
);

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

pub static HARNESSES: &[HarnessSpec] = &[
    // -----------------------------------------------------------------------
    // omc — oh-my-claudecode → Claude Code
    // -----------------------------------------------------------------------
    HarnessSpec {
        id: "omc",
        display_name: "oh-my-claudecode",
        target_runtime: "Claude Code",
        package: PackageSpec::NpmGlobal {
            package: "oh-my-claude-sisyphus",
        },
        detect_binaries: &["omc"],
        launch_binary: None,
        isolation: IsolationSpec {
            subdir: "omc",
            spoof_home: true,
            home_subdirs: &[
                ".claude",
                ".claude/hooks",
                ".claude/agents",
                ".claude/commands",
                ".claude/skills",
                ".claude/hud",
                ".claude/hud/lib",
            ],
            static_envs: &[
                ("CLAUDE_CONFIG_DIR", "{home}/.claude"),
                ("CLAUDE_CODE_TMPDIR", "{tmp}"),
                ("CLAUDE_CODE_DEBUG_LOGS_DIR", "{state}/logs"),
                ("DISABLE_UPDATES", "1"),
                ("DISABLE_AUTOUPDATER", "1"),
                ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1"),
                ("ENABLE_CLAUDEAI_MCP_SERVERS", "false"),
                ("CLAUDE_CODE_SUBPROCESS_ENV_SCRUB", "1"),
            ],
            seed_files: &[SeedFile {
                path: "{home}/.claude/settings.json",
                content: OMC_CLAUDE_SETTINGS_JSON,
                overwrite: false,
                mode: None,
            }],
            caveat: Some(
                "omc harness: Claude Code with oh-my-claudecode plugin. Install omc first: hm harness install omc",
            ),
        },
    },
    // -----------------------------------------------------------------------
    // omx — oh-my-codex → Codex CLI
    // -----------------------------------------------------------------------
    HarnessSpec {
        id: "omx",
        display_name: "oh-my-codex",
        target_runtime: "Codex CLI",
        package: PackageSpec::NpmGlobal {
            package: "oh-my-codex",
        },
        detect_binaries: &["omx"],
        launch_binary: None,
        isolation: IsolationSpec {
            subdir: "omx",
            spoof_home: true,
            home_subdirs: &[".codex"],
            static_envs: &[
                ("CODEX_HOME", "{home}/.codex"),
                ("OMX_ROOT", "{home}/.omx"),
                ("OMX_STATE_ROOT", "{state}/omx"),
                ("OMX_TEAM_STATE_ROOT", "{state}/omx-team"),
            ],
            seed_files: &[SeedFile {
                path: "{home}/.codex/config.toml",
                content: CODEX_BASE_CONFIG_TOML,
                overwrite: false,
                mode: None,
            }],
            caveat: Some(
                "omx harness: Codex CLI with oh-my-codex. Run `omx setup` after first launch if needed.",
            ),
        },
    },
    // -----------------------------------------------------------------------
    // omo — oh-my-openagent → OpenCode
    // -----------------------------------------------------------------------
    HarnessSpec {
        id: "omo",
        display_name: "oh-my-openagent",
        target_runtime: "OpenCode",
        package: PackageSpec::BunxInstaller {
            package: "oh-my-openagent",
            args: &["install"],
        },
        detect_binaries: &["omo"],
        launch_binary: None,
        isolation: IsolationSpec {
            subdir: "omo",
            spoof_home: true,
            home_subdirs: &[
                ".config/opencode",
                ".local/share/opencode",
                ".cache/opencode",
                ".local/state/opencode",
            ],
            static_envs: &[
                ("XDG_CONFIG_HOME", "{home}/.config"),
                ("XDG_DATA_HOME", "{home}/.local/share"),
                ("XDG_CACHE_HOME", "{home}/.cache"),
                ("XDG_STATE_HOME", "{home}/.local/state"),
                ("OPENCODE_CONFIG_DIR", "{home}/.config/opencode"),
                ("OPENCODE_DISABLE_AUTOUPDATE", "1"),
                ("OPENCODE_DISABLE_PROJECT_CONFIG", "1"),
                ("OPENCODE_PURE", "1"),
            ],
            seed_files: &[],
            caveat: Some("omo harness: OpenCode with oh-my-openagent plugin."),
        },
    },
    // -----------------------------------------------------------------------
    // lazycodex → Codex CLI (wrapper binary)
    // -----------------------------------------------------------------------
    HarnessSpec {
        id: "lazycodex",
        display_name: "lazycodex",
        target_runtime: "Codex CLI",
        package: PackageSpec::NpxInstaller {
            package: "lazycodex-ai",
            args: &["install"],
        },
        detect_binaries: &["lazycodex-ai"],
        launch_binary: Some("lazycodex-ai"),
        isolation: IsolationSpec {
            subdir: "lazycodex",
            spoof_home: true,
            home_subdirs: &[".codex"],
            static_envs: &[("CODEX_HOME", "{home}/.codex")],
            seed_files: &[SeedFile {
                path: "{home}/.codex/config.toml",
                content: CODEX_BASE_CONFIG_TOML,
                overwrite: false,
                mode: None,
            }],
            caveat: None,
        },
    },
    // -----------------------------------------------------------------------
    // ouroboros → Codex CLI (default target; multi-runtime support deferred)
    // -----------------------------------------------------------------------
    HarnessSpec {
        id: "ouroboros",
        display_name: "ouroboros",
        target_runtime: "Codex CLI",
        package: PackageSpec::PythonTool {
            package: "ouroboros-ai",
        },
        detect_binaries: &["ouroboros"],
        launch_binary: None,
        isolation: IsolationSpec {
            subdir: "ouroboros",
            spoof_home: true,
            home_subdirs: &[".codex", ".ouroboros"],
            static_envs: &[("CODEX_HOME", "{home}/.codex")],
            seed_files: &[SeedFile {
                path: "{home}/.codex/config.toml",
                content: CODEX_BASE_CONFIG_TOML,
                overwrite: false,
                mode: None,
            }],
            caveat: Some(
                "ouroboros harness: Codex CLI with ouroboros workflow engine. Run `ouroboros setup --runtime codex` after install.",
            ),
        },
    },
];

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
