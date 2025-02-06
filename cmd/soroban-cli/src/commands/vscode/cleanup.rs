use super::shared::{Args, Error};

#[derive(Debug, clap::Parser, Clone)]
pub struct Cmd {
    #[command(flatten)]
    args: Args,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        // TODO: remove the schema from vscode settings and remove the schema file
        Ok(())
    }
}
