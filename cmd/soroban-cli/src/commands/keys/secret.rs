use clap::arg;

use crate::config::{
    key::{self, Key},
    locator,
    secret::Secret,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),

    #[error(transparent)]
    Key(#[from] key::Error),

    #[error("identity is not tied to a seed phrase")]
    UnknownSeedPhrase,
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
#[command(name = "secret", alias = "show")]
pub struct Cmd {
    /// Name of identity to lookup, default is test identity
    pub name: String,

    /// Output seed phrase instead of private key
    #[arg(long, conflicts_with = "hd_path")]
    pub phrase: bool,

    /// If identity is a seed phrase use this hd path, default is 0
    #[arg(long, conflicts_with = "phrase")]
    pub hd_path: Option<usize>,

    #[command(flatten)]
    pub locator: locator::Args,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        if self.phrase {
            println!("{}", self.seed_phrase()?);
        } else {
            println!("{}", self.private_key()?);
        }

        Ok(())
    }

    pub fn seed_phrase(&self) -> Result<String, Error> {
        let key = self.locator.read_identity(&self.name)?;

        if let Key::Secret(Secret::SeedPhrase { seed_phrase }) = key {
            Ok(seed_phrase)
        } else {
            Err(Error::UnknownSeedPhrase)
        }
    }

    pub fn private_key(&self) -> Result<stellar_strkey::ed25519::PrivateKey, Error> {
        Ok(self
            .locator
            .read_identity(&self.name)?
            .private_key(self.hd_path)?)
    }
}
