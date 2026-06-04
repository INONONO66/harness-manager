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

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Detect => {
            detect::run_detect()?;
        }

        Commands::Harness { action } => match action {
            HarnessAction::List => {
                let detected = harnesses::detect::detect_all();
                harnesses::detect::render_table(&detected);
            }
            HarnessAction::Install { name } => {
                harnesses::install::install(&name)?;
            }
            HarnessAction::Update { name } => {
                harnesses::install::update(&name)?;
            }
            HarnessAction::Remove { name, purge } => {
                harnesses::install::remove(&name, purge)?;
            }
        },

        Commands::Auth { action } => match action {
            AuthAction::Status => {
                auth::run_auth_status()?;
            }
            AuthAction::Login { runtime } => {
                auth::login::run_auth_login(&runtime)?;
            }
        },

        Commands::Inject { action } => match action {
            InjectAction::Plan { target, profile } => {
                inject::run_inject_plan(&target, &profile)?;
            }
            InjectAction::Apply {
                target,
                profile,
                persist,
            } => {
                inject::run_inject_apply(&target, &profile, persist)?;
            }
            InjectAction::Reset { target } => {
                inject::run_inject_reset(&target)?;
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
            let _ = harness;
            launch::run_use(
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
            // `hm omx` is sugar for `hm use omx -- <rest>`
            let name = &args[0];
            if harnesses::find_harness_spec(name).is_none() {
                anyhow::bail!(
                    "unknown command: '{}'. Run `hm --help`, `hm detect`, or `hm harness list`.",
                    name
                );
            }
            let extra: Vec<String> = args[1..].to_vec();
            launch::run_use(name, None, false, false, &extra)?;
        }
    }

    Ok(())
}
