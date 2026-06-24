use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "hm",
    about = "Agent Runtime Manager — detect, manage, and launch AI coding agent runtimes",
    long_about = "Agent Runtime Manager — keep AI coding agent runtimes, auth state, proxy profiles, and harness isolation manageable from one command layer.\n\nStart with:\n  hm detect\n  hm auth status\n  hm harness list\n  hm use codex --profile proxy -- --help",
    version,
    propagate_version = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Install built-in harnesses (runtimes and harnesses are compiled into hm)
    Init {
        /// Install every harness whose `package.kind` is not `manual`
        #[arg(long)]
        install: bool,
    },

    /// Detect installed agent runtimes
    #[command(alias = "ls")]
    Detect,

    /// Manage harness installations (install, update, remove, list)
    #[command(alias = "h")]
    Harness {
        #[command(subcommand)]
        action: HarnessAction,
    },

    /// Manage authentication across runtimes
    #[command(alias = "a")]
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },

    /// Preview injection profile effects
    #[command(alias = "profile")]
    Inject {
        #[command(subcommand)]
        action: InjectAction,
    },

    /// Manage hm's local secret store
    #[command(alias = "secrets")]
    Secret {
        #[command(subcommand)]
        action: SecretAction,
    },

    /// Launch a runtime or registered harness with profile injection
    #[command(alias = "run")]
    Use {
        /// Runtime or harness to launch (claude, codex, gjc, grok, opencode, pi, or a harness id)
        runtime: String,

        /// Profile from ~/.config/hm/config.toml to inject
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
    #[command(alias = "list")]
    Status,

    /// Delegate login to a runtime's native auth flow
    Login {
        /// Runtime to log in (claude, codex, gjc, grok, opencode, pi)
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
    #[command(alias = "ls")]
    List,
    /// Install a harness
    Install {
        /// Registered harness name
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
