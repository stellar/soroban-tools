pub mod cleanup;
pub mod setup;

pub mod shared;
pub use shared::Error;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Setup Vs Code to use transaction schema
    Setup(setup::Cmd),
    // /// Remove the schema from vscode
    // Cleanup(cleanup::Cmd),
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        match &self {
            Cmd::Setup(cmd) => cmd.run()?,
            // Cmd::Cleanup(cmd) => cmd.run()?,
        }
        Ok(())
    }
}
