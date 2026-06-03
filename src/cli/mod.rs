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

        /// Extra arguments to pass to the runtime
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
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
