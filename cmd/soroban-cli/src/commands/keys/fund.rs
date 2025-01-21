use clap::command;

use crate::{commands::global, config::network, print::Print};

use super::address;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Address(#[from] address::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub network: network::Args,
    /// Address to fund
    #[command(flatten)]
    pub address: address::Cmd,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
        let addr = self.address.public_key()?;
        let network = self.network.get(&self.address.locator)?;
        network.fund_address(&addr).await?;
        print.checkln(format!(
            "Account {:?} funded on {:?}",
            self.address.name, network.network_passphrase
        ));
        Ok(())
    }
}
