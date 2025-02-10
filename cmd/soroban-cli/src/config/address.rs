use std::{
    fmt::{self, Display, Formatter},
    str::FromStr,
};

use crate::{
    signer::{self, ledger},
    xdr,
};

use super::{key, locator, secret};

/// Address can be either a public key or eventually an alias of a address.
#[derive(Clone, Debug)]
pub enum UnresolvedMuxedAccount {
    Resolved(xdr::MuxedAccount),
    AliasOrSecret(String),
    Ledger(u32),
}

impl Default for UnresolvedMuxedAccount {
    fn default() -> Self {
        UnresolvedMuxedAccount::AliasOrSecret(String::default())
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    Secret(#[from] secret::Error),
    #[error(transparent)]
    Signer(#[from] signer::Error),
    #[error(transparent)]
    Key(#[from] key::Error),
    #[error("Address cannot be used to sign {0}")]
    CannotSign(xdr::MuxedAccount),
    #[error("Ledger cannot reveal private keys")]
    LedgerPrivateKeyRevealNotSupported,
    #[error("Invalid key name: {0}\n `ledger` is not allowed")]
    LedgerIsInvalidKeyName(String),
    #[error("Invalid key name: {0}\n only alphanumeric characters, underscores (_), and hyphens (-) are allowed.")]
    InvalidKeyNameCharacters(String),
    #[error("Invalid key name: {0}\n keys cannot exceed 250 characters")]
    InvalidKeyNameLength(String),
    #[error("Invalid key name: {0}\n keys cannot be the word \"ledger\"")]
    InvalidKeyName(String),
    #[error("Ledger not supported in this context")]
    LedgerNotSupported,
}

impl FromStr for UnresolvedMuxedAccount {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.starts_with("ledger") {
            if let Some(ledger) = parse_ledger(value) {
                return Ok(UnresolvedMuxedAccount::Ledger(ledger));
            }
        }
        Ok(xdr::MuxedAccount::from_str(value).map_or_else(
            |_| UnresolvedMuxedAccount::AliasOrSecret(value.to_string()),
            UnresolvedMuxedAccount::Resolved,
        ))
    }
}

fn parse_ledger(value: &str) -> Option<u32> {
    let vals: Vec<_> = value.split(':').collect();
    if vals.len() > 2 {
        return None;
    }
    if vals.len() == 1 {
        return Some(0);
    }
    vals[1].parse().ok()
}

impl UnresolvedMuxedAccount {
    pub async fn resolve_muxed_account(
        &self,
        locator: &locator::Args,
        hd_path: Option<usize>,
    ) -> Result<xdr::MuxedAccount, Error> {
        match self {
            UnresolvedMuxedAccount::Ledger(hd_path) => Ok(xdr::MuxedAccount::Ed25519(
                ledger(*hd_path).await?.public_key().await?.0.into(),
            )),
            UnresolvedMuxedAccount::Resolved(_) | UnresolvedMuxedAccount::AliasOrSecret(_) => {
                self.resolve_muxed_account_sync(locator, hd_path)
            }
        }
    }

    pub fn resolve_muxed_account_sync(
        &self,
        locator: &locator::Args,
        hd_path: Option<usize>,
    ) -> Result<xdr::MuxedAccount, Error> {
        match self {
            UnresolvedMuxedAccount::Resolved(muxed_account) => Ok(muxed_account.clone()),
            UnresolvedMuxedAccount::AliasOrSecret(alias_or_secret) => {
                Ok(locator.read_key(alias_or_secret)?.muxed_account(hd_path)?)
            }
            UnresolvedMuxedAccount::Ledger(_) => Err(Error::LedgerNotSupported),
        }
    }

    pub fn resolve_secret(&self, locator: &locator::Args) -> Result<secret::Secret, Error> {
        match &self {
            UnresolvedMuxedAccount::Resolved(muxed_account) => {
                Err(Error::CannotSign(muxed_account.clone()))
            }
            UnresolvedMuxedAccount::AliasOrSecret(alias_or_secret) => {
                Ok(locator.read_key(alias_or_secret)?.try_into()?)
            }
            UnresolvedMuxedAccount::Ledger(_) => Err(Error::LedgerPrivateKeyRevealNotSupported),
        }
    }
}

#[derive(Clone, Debug)]
pub struct KeyName(pub String);

impl std::ops::Deref for KeyName {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::str::FromStr for KeyName {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if !s.chars().all(allowed_char) {
            return Err(Error::InvalidKeyNameCharacters(s.to_string()));
        }
        if s == "ledger" {
            return Err(Error::InvalidKeyName(s.to_string()));
        }
        if s.len() > 250 {
            return Err(Error::InvalidKeyNameLength(s.to_string()));
        }
        Ok(KeyName(s.to_string()))
    }
}

impl Display for KeyName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

fn allowed_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}
