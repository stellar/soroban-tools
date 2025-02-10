use crate::{
    commands::global,
    config::{locator, network, sign_with},
    xdr::{self, Limits, WriteXdr},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    XdrArgs(#[from] super::xdr::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    SignWith(#[from] sign_with::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub sign_with: sign_with::Args,
    #[command(flatten)]
    pub network: network::Args,
    #[command(flatten)]
    pub locator: locator::Args,
}

impl Cmd {
    #[allow(clippy::unused_async)]
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let tx_env = super::xdr::tx_envelope_from_stdin()?;
        let tx_env_signed = self
            .sign_with
            .sign_tx_env(
                &tx_env,
                &self.locator,
                &self.network.get(&self.locator)?,
                global_args.quiet,
            )
            .await?;
        println!("{}", tx_env_signed.to_xdr_base64(Limits::none())?);
        Ok(())
    }
}
