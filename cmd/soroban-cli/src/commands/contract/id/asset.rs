use clap::{arg, command, Parser};

use crate::config;

use crate::tx::builder;
use crate::utils::contract_id_hash_from_asset;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// ID of the Stellar classic asset to wrap, e.g. "USDC:G...5"
    #[arg(long)]
    pub asset: builder::Asset,

    #[command(flatten)]
    pub config: config::ArgsLocatorAndNetwork,
}
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    ConfigError(#[from] config::Error),
    #[error(transparent)]
    Xdr(#[from] crate::xdr::Error),
    #[error(transparent)]
    Asset(#[from] builder::asset::Error),
}
impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        println!("{}", self.contract_address()?);
        Ok(())
    }

    pub fn contract_address(&self) -> Result<stellar_strkey::Contract, Error> {
        let network = self.config.get_network()?;
        let contract_id = contract_id_hash_from_asset(
            &self.asset.resolve(&self.config.locator)?,
            &network.network_passphrase,
        );
        Ok(stellar_strkey::Contract(contract_id.0))
    }
}
