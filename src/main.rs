mod auth;
mod cli;
mod config;
mod detect;
mod harnesses;
mod inject;
mod isolation;
mod launch;
mod runtimes;
mod secrets;

use clap::Parser;
use cli::{AuthAction, Cli, Commands, HarnessAction, InjectAction, SecretAction};

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

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let runtimes = runtimes::registry::RuntimeRegistry::load()?;

    match cli.command {
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
