use crate::{
    signer::{self, types::sign_tx_env},
    xdr::TransactionEnvelope,
};
use clap::arg;

use super::{
    locator,
    network::{self, Network},
    secret::{self, Secret},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Signer(#[from] signer::types::Error),
    #[error(transparent)]
    Secret(#[from] secret::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    Rpc(#[from] soroban_rpc::Error),
    #[error("No sign with key provided")]
    NoSignWithKey,
    #[error(transparent)]
    StrKey(#[from] stellar_strkey::DecodeError),
}

#[derive(Debug, clap::Args, Clone, Default)]
#[group(skip)]
pub struct Args {
    /// Sign with a local key. Can be an identity (--sign-with-key alice), a secret key (--sign-with-key SC36…), or a seed phrase (--sign-with-key "kite urban…"). If using seed phrase, `--hd-path` defaults to the `0` path.
    #[arg(long, conflicts_with = "sign_with_lab", env = "STELLAR_SIGN_WITH_KEY")]
    pub sign_with_key: Option<String>,
    /// Sign with labratory
    #[arg(
        long,
        conflicts_with = "sign_with_key",
        env = "STELLAR_SIGN_WITH_LAB",
        hide = true
    )]
    pub sign_with_lab: bool,

    #[arg(long, conflicts_with = "sign_with_lab")]
    /// If using a seed phrase to sign, sets which hierarchical deterministic path to use, e.g. `m/44'/148'/{hd_path}`. Example: `--hd-path 1`. Default: `0`
    pub hd_path: Option<usize>,
}

impl Args {
    pub fn secret(&self, locator: &locator::Args) -> Result<Secret, Error> {
        let account = self.sign_with_key.as_deref().ok_or(Error::NoSignWithKey)?;
        Ok(locator.account(account)?)
    }

    pub async fn sign_tx_env(
        &self,
        tx: TransactionEnvelope,
        locator: &locator::Args,
        network: &Network,
        quiet: bool,
    ) -> Result<TransactionEnvelope, Error> {
        let secret = self.secret(locator)?;
        let signer = secret.signer(self.hd_path, false, quiet)?;
        Ok(sign_tx_env(&signer, tx, network).await?)
    }
}
