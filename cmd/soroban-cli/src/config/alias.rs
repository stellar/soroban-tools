use std::{collections::HashMap, convert::Infallible, str::FromStr};

use serde::{Deserialize, Serialize};

use super::locator;

#[derive(Serialize, Deserialize, Default)]
pub struct Data {
    pub ids: HashMap<String, String>,
}

/// Address can be either a contract address, C.. or eventually an alias of a contract address.
#[derive(Clone, Debug)]
pub enum UnresolvedContract {
    Resolved(stellar_strkey::Contract),
    Alias(String),
}

impl Default for UnresolvedContract {
    fn default() -> Self {
        UnresolvedContract::Alias(String::default())
    }
}

impl FromStr for UnresolvedContract {
    type Err = Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(stellar_strkey::Contract::from_str(value).map_or_else(
            |_| UnresolvedContract::Alias(value.to_string()),
            UnresolvedContract::Resolved,
        ))
    }
}

impl UnresolvedContract {
    pub fn resolve_contract_id(
        &self,
        locator: &locator::Args,
        network_passphrase: &str,
    ) -> Result<stellar_strkey::Contract, locator::Error> {
        match self {
            UnresolvedContract::Resolved(contract) => Ok(*contract),
            UnresolvedContract::Alias(alias) => {
                Self::resolve_alias(alias, locator, network_passphrase)
            }
        }
    }

    pub fn resolve_alias(
        alias: &str,
        locator: &locator::Args,
        network_passphrase: &str,
    ) -> Result<stellar_strkey::Contract, locator::Error> {
        locator
            .get_contract_id(alias, network_passphrase)?
            .ok_or_else(|| locator::Error::ContractNotFound(alias.to_owned()))
    }
}
