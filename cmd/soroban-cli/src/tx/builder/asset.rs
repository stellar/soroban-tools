use std::str::FromStr;

use crate::{
    config::{address, locator},
    xdr::{self, AlphaNum12, AlphaNum4, AssetCode},
};

#[derive(Clone, Debug)]
pub enum Asset {
    Asset(AssetCode, address::UnresolvedMuxedAccount),
    Native,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("cannot parse asset: {0}, expected format: 'native' or 'code:issuer'")]
    CannotParseAsset(String),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error(transparent)]
    Address(#[from] address::Error),
}

impl FromStr for Asset {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value == "native" {
            return Ok(Asset::Native);
        }
        let mut iter = value.splitn(2, ':');
        let (Some(code), Some(issuer), None) = (iter.next(), iter.next(), iter.next()) else {
            return Err(Error::CannotParseAsset(value.to_string()));
        };
        Ok(Asset::Asset(code.parse()?, issuer.parse()?))
    }
}

impl Asset {
    pub fn resolve(&self, locator: &locator::Args) -> Result<xdr::Asset, Error> {
        Ok(match self {
            Asset::Asset(code, issuer) => {
                let issuer = issuer.resolve_muxed_account(locator, None)?.account_id();
                match code.clone() {
                    AssetCode::CreditAlphanum4(asset_code) => {
                        xdr::Asset::CreditAlphanum4(AlphaNum4 { asset_code, issuer })
                    }
                    AssetCode::CreditAlphanum12(asset_code) => {
                        xdr::Asset::CreditAlphanum12(AlphaNum12 { asset_code, issuer })
                    }
                }
            }
            Asset::Native => xdr::Asset::Native,
        })
    }
}
