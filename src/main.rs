#[cfg(windows)]
compile_error!("hm supports macOS and Linux only; Windows is not supported.");

#[cfg(windows)]
fn main() {}

#[cfg(not(windows))]
mod auth;
#[cfg(not(windows))]
mod cli;
#[cfg(not(windows))]
mod config;
#[cfg(not(windows))]
mod detect;
#[cfg(not(windows))]
mod harnesses;
#[cfg(not(windows))]
mod init;
#[cfg(not(windows))]
mod inject;
#[cfg(not(windows))]
mod isolation;
#[cfg(not(windows))]
mod launch;
#[cfg(not(windows))]
mod runtimes;
#[cfg(not(windows))]
mod secrets;

#[cfg(not(windows))]
use clap::Parser;
#[cfg(not(windows))]
use cli::{AuthAction, Cli, Commands, HarnessAction, InjectAction, SecretAction};

#[cfg(not(windows))]
fn harness_labels(registry: &harnesses::registry::HarnessRegistry) -> String {
    registry
        .specs()
        .iter()
        .map(|spec| {
            if spec.aliases.is_empty() {
                spec.id.clone()
            } else {
                format!("{} ({})", spec.id, spec.aliases.join(", "))
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(not(windows))]
fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    if let Commands::Init { install } = cli.command {
        return init::run_init(install);
    }

    let runtimes = runtimes::registry::RuntimeRegistry::load()?;

    match cli.command {
        Commands::Init { .. } => unreachable!("handled above before registry load"),

        Commands::Detect => {
            detect::run_detect(&runtimes)?;
        }

        Commands::Harness { action } => match action {
            HarnessAction::List => {
                let registry = harnesses::load_registry(&runtimes)?;
                let detected = harnesses::detect::detect_all(&registry);
                harnesses::detect::render_table(&detected);
            }
            HarnessAction::Install { name } => {
                let registry = harnesses::load_registry(&runtimes)?;
                harnesses::install::install(&registry, &name)?;
            }
            HarnessAction::Update { name } => {
                let registry = harnesses::load_registry(&runtimes)?;
                harnesses::install::update(&registry, &name)?;
            }
            HarnessAction::Remove { name, purge } => {
                let registry = harnesses::load_registry(&runtimes)?;
                harnesses::install::remove(&registry, &name, purge)?;
            }
        },

        Commands::Auth { action } => match action {
            AuthAction::Status => {
                auth::run_auth_status(&runtimes)?;
            }
            AuthAction::Login { runtime } => {
                auth::login::run_auth_login(&runtimes, &runtime)?;
            }
        },

        Commands::Inject { action } => match action {
            InjectAction::Plan { target, profile } => {
                let registry = harnesses::load_registry(&runtimes)?;
                inject::run_inject_plan(&runtimes, &registry, &target, &profile)?;
            }
        },

        Commands::Secret { action } => match action {
            SecretAction::Set { name } => {
                secrets::run_secret_set(&name)?;
            }
            SecretAction::Get { name } => {
                secrets::run_secret_get(&name)?;
            }
            SecretAction::List => {
                secrets::run_secret_list()?;
            }
            SecretAction::Rm { name } => {
                secrets::run_secret_rm(&name)?;
            }
        },

        Commands::Use {
            runtime,
            profile,
            print_env,
            allow_keychain,
            harness,
            args,
        } => {
            let registry = harnesses::load_registry(&runtimes)?;
            if harness && registry.find(&runtime).is_none() {
                anyhow::bail!(
                    "--harness target '{}' is not registered. Available harnesses: {}. Run `hm harness list` for status.",
                    runtime,
                    harness_labels(&registry)
                );
            }
            launch::run_use(
                &runtimes,
                &registry,
                &runtime,
                profile.as_deref(),
                print_env,
                allow_keychain,
                &args,
            )?;
        }

        Commands::External(args) => {
            if args.is_empty() {
                anyhow::bail!("unexpected empty external subcommand");
            }
            let name = &args[0];
            let registry = harnesses::load_registry(&runtimes)?;
            if registry.find(name).is_none() {
                anyhow::bail!(
                    "unknown command: '{}'. Try `hm --help`, a runtime via `hm use <runtime>`, or one of these harnesses: {}.",
                    name,
                    harness_labels(&registry)
                );
            }
            let extra: Vec<String> = args[1..].to_vec();
            launch::run_use(&runtimes, &registry, name, None, false, false, &extra)?;
        }
    }

    Ok(())
}
