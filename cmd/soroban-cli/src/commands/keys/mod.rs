use crate::commands::global;
use clap::Parser;

pub mod add;
pub mod address;
pub mod default;
pub mod fund;
pub mod generate;
pub mod ls;
pub mod rm;
pub mod secret;

#[derive(Debug, Parser)]
pub enum Cmd {
    /// Add a new identity (keypair, ledger, OS specific secure store)
    Add(add::Cmd),

    /// Given an identity return its address (public key)
    Address(address::Cmd),

    /// Fund an identity on a test network
    Fund(fund::Cmd),

    /// Generate a new identity with a seed phrase, currently 12 words
    Generate(generate::Cmd),

    /// List identities
    Ls(ls::Cmd),

    /// Remove an identity
    Rm(rm::Cmd),

    /// Output an identity's secret key
    Secret(secret::Cmd),

    /// Set the default identity that will be used on all commands.
    /// This allows you to skip `--source-account` or setting a environment
    /// variable, while reusing this value in all commands that require it.
    #[command(name = "use")]
    Default(default::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Add(#[from] add::Error),

    #[error(transparent)]
    Address(#[from] address::Error),

    #[error(transparent)]
    Fund(#[from] fund::Error),

    #[error(transparent)]
    Generate(#[from] generate::Error),

    #[error(transparent)]
    Rm(#[from] rm::Error),

    #[error(transparent)]
    Ls(#[from] ls::Error),

    #[error(transparent)]
    Show(#[from] secret::Error),

    #[error(transparent)]
    Default(#[from] default::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Add(cmd) => cmd.run(global_args)?,
            Cmd::Address(cmd) => cmd.run()?,
            Cmd::Fund(cmd) => cmd.run(global_args).await?,
            Cmd::Generate(cmd) => cmd.run(global_args).await?,
            Cmd::Ls(cmd) => cmd.run()?,
            Cmd::Rm(cmd) => cmd.run()?,
            Cmd::Secret(cmd) => cmd.run()?,
            Cmd::Default(cmd) => cmd.run(global_args)?,
        };
        Ok(())
    }
}
