mod auth;
mod cli;
mod config;
mod detect;
mod inject;
mod isolation;
mod launch;
mod runtimes;
mod secrets;

use clap::Parser;
use cli::{AuthAction, Cli, Commands, InjectAction, SecretAction};

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Detect => {
            detect::run_detect()?;
        }

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
            args,
        } => {
            launch::run_use(
                &runtime,
                profile.as_deref(),
                print_env,
                allow_keychain,
                &args,
            )?;
        }
    }

    Ok(())
}
