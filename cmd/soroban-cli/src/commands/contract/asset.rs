use crate::commands::global;

use super::{deploy, id};

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Get Id of builtin Soroban Asset Contract. Deprecated, use `stellar contract id asset` instead
    Id(id::asset::Cmd),
    /// Deploy builtin Soroban Asset Contract
    Deploy(deploy::asset::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Id(#[from] id::asset::Error),
    #[error(transparent)]
    Deploy(#[from] deploy::asset::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match &self {
            Cmd::Id(id) => id.run()?,
            Cmd::Deploy(asset) => asset.run(global_args).await?,
        }
        Ok(())
    }
}
