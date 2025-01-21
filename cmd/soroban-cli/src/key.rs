use crate::xdr::{
    self, LedgerKey, LedgerKeyContractCode, LedgerKeyContractData, Limits, ReadXdr, ScAddress,
    ScVal,
};
use crate::{
    commands::contract::Durability,
    config::{alias, locator, network::Network},
    wasm,
};
use clap::arg;
use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Spec(#[from] soroban_spec_tools::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error("cannot parse contract ID {0}: {1}")]
    CannotParseContractId(String, stellar_strkey::DecodeError),
    #[error(transparent)]
    Wasm(#[from] wasm::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
}

#[derive(Debug, clap::Args, Clone)]
#[group(skip)]
pub struct Args {
    /// Contract ID to which owns the data entries.
    /// If no keys provided the Contract's instance will be extended
    #[arg(
        long = "id",
        required_unless_present = "wasm",
        required_unless_present = "wasm_hash"
    )]
    pub contract_id: Option<alias::UnresolvedContract>,
    /// Storage key (symbols only)
    #[arg(long = "key", conflicts_with = "key_xdr")]
    pub key: Option<Vec<String>>,
    /// Storage key (base64-encoded XDR)
    #[arg(long = "key-xdr", conflicts_with = "key")]
    pub key_xdr: Option<Vec<String>>,
    /// Path to Wasm file of contract code to extend
    #[arg(
        long,
        conflicts_with = "contract_id",
        conflicts_with = "key",
        conflicts_with = "key_xdr",
        conflicts_with = "wasm_hash"
    )]
    pub wasm: Option<PathBuf>,
    /// Path to Wasm file of contract code to extend
    #[arg(
        long,
        conflicts_with = "contract_id",
        conflicts_with = "key",
        conflicts_with = "key_xdr",
        conflicts_with = "wasm"
    )]
    pub wasm_hash: Option<String>,
    /// Storage entry durability
    #[arg(long, value_enum, default_value = "persistent")]
    pub durability: Durability,
}

impl Args {
    pub fn parse_keys(
        &self,
        locator: &locator::Args,
        Network {
            network_passphrase, ..
        }: &Network,
    ) -> Result<Vec<LedgerKey>, Error> {
        let keys = if let Some(keys) = &self.key {
            keys.iter()
                .map(|key| {
                    Ok(soroban_spec_tools::from_string_primitive(
                        key,
                        &xdr::ScSpecTypeDef::Symbol,
                    )?)
                })
                .collect::<Result<Vec<_>, Error>>()?
        } else if let Some(keys) = &self.key_xdr {
            keys.iter()
                .map(|s| Ok(ScVal::from_xdr_base64(s, Limits::none())?))
                .collect::<Result<Vec<_>, Error>>()?
        } else if let Some(wasm) = &self.wasm {
            return Ok(vec![crate::wasm::Args { wasm: wasm.clone() }.try_into()?]);
        } else if let Some(wasm_hash) = &self.wasm_hash {
            return Ok(vec![LedgerKey::ContractCode(LedgerKeyContractCode {
                hash: xdr::Hash(
                    soroban_spec_tools::utils::contract_id_from_str(wasm_hash)
                        .map_err(|e| Error::CannotParseContractId(wasm_hash.clone(), e))?,
                ),
            })]);
        } else {
            vec![ScVal::LedgerKeyContractInstance]
        };
        let contract = self
            .contract_id
            .as_ref()
            .unwrap()
            .resolve_contract_id(locator, network_passphrase)?;

        Ok(keys
            .into_iter()
            .map(|key| {
                LedgerKey::ContractData(LedgerKeyContractData {
                    contract: ScAddress::Contract(xdr::Hash(contract.0)),
                    durability: (&self.durability).into(),
                    key,
                })
            })
            .collect())
    }
}
