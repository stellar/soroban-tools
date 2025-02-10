use clap::command;

use crate::{commands::global, config::network, print::Print};

use super::public_key;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Address(#[from] public_key::Error),
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
    pub address: public_key::Cmd,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);
        let addr = self.address.public_key().await?;
        let network = self.network.get(&self.address.locator)?;
        network.fund_address(&addr).await?;
        print.checkln(format!(
            "Account {:?} funded on {:?}",
            self.address.name, network.network_passphrase
        ));
        Ok(())
    }
}
