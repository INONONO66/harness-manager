use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "hm",
    about = "Agent Runtime Manager — detect, manage, and launch AI coding agent runtimes",
    version,
    propagate_version = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Detect installed agent runtimes
    Detect,

    /// Manage harness installations (install, update, remove, list)
    Harness {
        #[command(subcommand)]
        action: HarnessAction,
    },

    /// Manage authentication across runtimes
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },

    /// Manage injection profiles and apply/reset persistent injections
    Inject {
        #[command(subcommand)]
        action: InjectAction,
    },

    /// Manage hm's local secret store
    Secret {
        #[command(subcommand)]
        action: SecretAction,
    },

    /// Launch a runtime with profile injection
    Use {
        /// Runtime to launch (claude, codex, opencode, pi)
        runtime: String,

        /// Profile to inject
        #[arg(short, long)]
        profile: Option<String>,

        /// Print the would-be environment to stdout and exit (for verification).
        /// Filesystem side-effects (isolation tree, seed files) still run.
        #[arg(long)]
        print_env: bool,

        /// For Claude only: allow OAuth/Keychain mode instead of apiKeyHelper isolation.
        #[arg(long)]
        allow_keychain: bool,

        /// Treat the runtime argument as a harness identifier.
        #[arg(long)]
        harness: bool,

        /// Extra arguments to pass to the runtime
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// External subcommand: matches registered harness IDs.
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Subcommand)]
pub enum AuthAction {
    /// Show auth status for all runtimes
    Status,

    /// Delegate login to a runtime's native auth flow
    Login {
        /// Runtime to login (claude, codex, opencode, pi)
        runtime: String,
    },
}

#[derive(Subcommand)]
pub enum InjectAction {
    /// Preview injection changes (dry-run)
    Plan {
        /// Runtime or "all"
        target: String,

        /// Profile to inject
        #[arg(short, long)]
        profile: String,
    },

    /// Apply persistent injection to config files
    Apply {
        /// Runtime to inject
        target: String,

        /// Profile to inject
        #[arg(short, long)]
        profile: String,

        /// Write to native config files
        #[arg(long)]
        persist: bool,
    },

    /// Reset persistent injections
    Reset {
        /// Runtime or "all"
        target: String,
    },
}

#[derive(Subcommand)]
pub enum SecretAction {
    /// Store a secret from stdin
    Set {
        /// Secret name (for Claude default: claude-api-key)
        name: String,
    },

    /// Print a secret to stdout
    Get {
        /// Secret name
        name: String,
    },

    /// List secret names (never values)
    List,

    /// Remove a secret
    Rm {
        /// Secret name
        name: String,
    },
}

#[derive(Subcommand)]
pub enum HarnessAction {
    /// List available harnesses and their install status
    List,
    /// Install a harness
    Install {
        /// Registered harness name.
        name: String,
    },
    /// Update an installed harness
    Update {
        /// Harness name
        name: String,
    },
    /// Remove an installed harness
    Remove {
        /// Harness name
        name: String,

        /// Also delete the harness isolation directory ($HM/runtimes/<id>/)
        #[arg(long)]
        purge: bool,
    },
}
