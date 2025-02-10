pub mod asset;
pub mod wasm;

#[derive(Debug, clap::Subcommand)]
pub enum Cmd {
    /// Deploy builtin Soroban Asset Contract
    Asset(asset::Cmd),
    /// Deploy normal Wasm Contract
    Wasm(wasm::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Asset(#[from] asset::Error),
    #[error(transparent)]
    Wasm(#[from] wasm::Error),
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        match &self {
            Cmd::Asset(asset) => asset.run()?,
            Cmd::Wasm(wasm) => wasm.run().await?,
        }
        Ok(())
    }
}
