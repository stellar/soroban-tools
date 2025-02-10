use clap::arg;

use crate::{
    commands::config::{address, locator},
    config::UnresolvedMuxedAccount,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Address(#[from] address::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Name of identity to lookup, default test identity used if not provided
    pub name: UnresolvedMuxedAccount,

    /// If identity is a seed phrase use this hd path, default is 0
    #[arg(long)]
    pub hd_path: Option<usize>,

    #[command(flatten)]
    pub locator: locator::Args,
}

impl Cmd {
    pub async fn run(&self) -> Result<(), Error> {
        println!("{}", self.public_key().await?);
        Ok(())
    }

    pub async fn public_key(&self) -> Result<stellar_strkey::ed25519::PublicKey, Error> {
        let muxed = self
            .name
            .resolve_muxed_account(&self.locator, self.hd_path)
            .await?;
        let bytes = match muxed {
            soroban_sdk::xdr::MuxedAccount::Ed25519(uint256) => uint256.0,
            soroban_sdk::xdr::MuxedAccount::MuxedEd25519(muxed_account) => muxed_account.ed25519.0,
        };
        Ok(stellar_strkey::ed25519::PublicKey(bytes))
    }
}
