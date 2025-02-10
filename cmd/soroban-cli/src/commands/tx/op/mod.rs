use super::global;

pub mod add;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Add Operation to a transaction
    #[command(subcommand)]
    Add(add::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Add(#[from] add::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match self {
            Cmd::Add(cmd) => cmd.run(global_args).await?,
        };
        Ok(())
    }
}
