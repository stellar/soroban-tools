use crate::commands::global;

pub mod add;
pub mod ls;
pub mod remove;
pub mod show;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Remove contract alias
    Remove(remove::Cmd),

    /// Add contract alias
    Add(add::Cmd),

    /// Show the contract id associated with a given alias
    Show(show::Cmd),

    /// List all aliases
    Ls(ls::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Remove(#[from] remove::Error),

    #[error(transparent)]
    Add(#[from] add::Error),

    #[error(transparent)]
    Show(#[from] show::Error),

    #[error(transparent)]
    Ls(#[from] ls::Error),
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        match &self {
            Cmd::Remove(remove) => remove.run(global_args)?,
            Cmd::Add(add) => add.run(global_args)?,
            Cmd::Show(show) => show.run(global_args)?,
            Cmd::Ls(ls) => ls.run()?,
        }
        Ok(())
    }
}
