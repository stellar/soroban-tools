use phf::phf_map;
use sha2::{Digest, Sha256};
use stellar_strkey::ed25519::PrivateKey;

use crate::xdr::{
    self, Asset, ContractIdPreimage, Hash, HashIdPreimage, HashIdPreimageContractId, Limits, ScMap,
    ScMapEntry, ScVal, Transaction, TransactionSignaturePayload,
    TransactionSignaturePayloadTaggedTransaction, WriteXdr,
};

pub use soroban_spec_tools::contract as contract_spec;

use crate::config::network::Network;

/// # Errors
///
/// Might return an error
pub fn contract_hash(contract: &[u8]) -> Result<Hash, xdr::Error> {
    Ok(Hash(Sha256::digest(contract).into()))
}

/// # Errors
///
/// Might return an error
pub fn transaction_hash(
    tx: &Transaction,
    network_passphrase: &str,
) -> Result<[u8; 32], xdr::Error> {
    let signature_payload = TransactionSignaturePayload {
        network_id: Hash(Sha256::digest(network_passphrase).into()),
        tagged_transaction: TransactionSignaturePayloadTaggedTransaction::Tx(tx.clone()),
    };
    Ok(Sha256::digest(signature_payload.to_xdr(Limits::none())?).into())
}

static EXPLORERS: phf::Map<&'static str, &'static str> = phf_map! {
    "Test SDF Network ; September 2015" => "https://stellar.expert/explorer/testnet",
    "Public Global Stellar Network ; September 2015" => "https://stellar.expert/explorer/public",
};

pub fn explorer_url_for_transaction(network: &Network, tx_hash: &str) -> Option<String> {
    EXPLORERS
        .get(&network.network_passphrase)
        .map(|base_url| format!("{base_url}/tx/{tx_hash}"))
}

pub fn explorer_url_for_contract(
    network: &Network,
    contract_id: &stellar_strkey::Contract,
) -> Option<String> {
    EXPLORERS
        .get(&network.network_passphrase)
        .map(|base_url| format!("{base_url}/contract/{contract_id}"))
}

/// # Errors
///
/// Might return an error
pub fn contract_id_from_str(
    contract_id: &str,
) -> Result<stellar_strkey::Contract, stellar_strkey::DecodeError> {
    Ok(
        if let Ok(strkey) = stellar_strkey::Contract::from_string(contract_id) {
            strkey
        } else {
            // strkey failed, try to parse it as a hex string, for backwards compatibility.
            stellar_strkey::Contract(
                soroban_spec_tools::utils::padded_hex_from_str(contract_id, 32)
                    .map_err(|_| stellar_strkey::DecodeError::Invalid)?
                    .try_into()
                    .map_err(|_| stellar_strkey::DecodeError::Invalid)?,
            )
        },
    )
}

/// # Errors
/// May not find a config dir
pub fn find_config_dir(mut pwd: std::path::PathBuf) -> std::io::Result<std::path::PathBuf> {
    loop {
        let stellar_dir = pwd.join(".stellar");
        let stellar_exists = stellar_dir.exists();

        let soroban_dir = pwd.join(".soroban");
        let soroban_exists = soroban_dir.exists();

        if stellar_exists && soroban_exists {
            tracing::warn!("the .stellar and .soroban config directories exist at path {pwd:?}, using the .stellar");
        }

        if stellar_exists {
            return Ok(stellar_dir);
        }

        if soroban_exists {
            return Ok(soroban_dir);
        }

        if !pwd.pop() {
            break;
        }
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "stellar directory not found",
    ))
}

pub(crate) fn into_signing_key(key: &PrivateKey) -> ed25519_dalek::SigningKey {
    let secret: ed25519_dalek::SecretKey = key.0;
    ed25519_dalek::SigningKey::from_bytes(&secret)
}

/// Used in tests
#[allow(unused)]
pub(crate) fn parse_secret_key(
    s: &str,
) -> Result<ed25519_dalek::SigningKey, stellar_strkey::DecodeError> {
    Ok(into_signing_key(&PrivateKey::from_string(s)?))
}

pub fn is_hex_string(s: &str) -> bool {
    s.chars().all(|s| s.is_ascii_hexdigit())
}

pub fn contract_id_hash_from_asset(
    asset: &Asset,
    network_passphrase: &str,
) -> stellar_strkey::Contract {
    let network_id = Hash(Sha256::digest(network_passphrase.as_bytes()).into());
    let preimage = HashIdPreimage::ContractId(HashIdPreimageContractId {
        network_id,
        contract_id_preimage: ContractIdPreimage::Asset(asset.clone()),
    });
    let preimage_xdr = preimage
        .to_xdr(Limits::none())
        .expect("HashIdPreimage should not fail encoding to xdr");
    stellar_strkey::Contract(Sha256::digest(preimage_xdr).into())
}

pub fn get_name_from_stellar_asset_contract_storage(storage: &ScMap) -> Option<String> {
    if let Some(ScMapEntry {
        val: ScVal::Map(Some(map)),
        ..
    }) = storage
        .iter()
        .find(|ScMapEntry { key, .. }| key == &ScVal::Symbol("METADATA".try_into().unwrap()))
    {
        if let Some(ScMapEntry {
            val: ScVal::String(name),
            ..
        }) = map
            .iter()
            .find(|ScMapEntry { key, .. }| key == &ScVal::Symbol("name".try_into().unwrap()))
        {
            Some(name.to_string())
        } else {
            None
        }
    } else {
        None
    }
}

pub mod http {
    use crate::commands::version;
    fn user_agent() -> String {
        format!("{}/{}", env!("CARGO_PKG_NAME"), version::pkg())
    }

    /// Creates and returns a configured `reqwest::Client`.
    ///
    /// # Panics
    ///
    /// Panics if the Client initialization fails.
    pub fn client() -> reqwest::Client {
        // Why we panic here:
        // 1. Client initialization failures are rare and usually indicate serious issues.
        // 2. The application cannot function properly without a working HTTP client.
        // 3. This simplifies error handling for callers, as they can assume a valid client.
        reqwest::Client::builder()
            .user_agent(user_agent())
            .build()
            .expect("Failed to build reqwest client")
    }

    /// Creates and returns a configured `reqwest::blocking::Client`.
    ///
    /// # Panics
    ///
    /// Panics if the Client initialization fails.
    pub fn blocking_client() -> reqwest::blocking::Client {
        reqwest::blocking::Client::builder()
            .user_agent(user_agent())
            .build()
            .expect("Failed to build reqwest blocking client")
    }
}

pub mod args {
    #[derive(thiserror::Error, Debug)]
    pub enum DeprecatedError<'a> {
        #[error("This argument has been removed and will be not be recognized by the future versions of CLI: {0}"
        )]
        RemovedArgument(&'a str),
    }

    #[macro_export]
    /// Mark argument as removed with an error to be printed when it's used.
    macro_rules! error_on_use_of_removed_arg {
        ($_type:ident, $message: expr) => {
            |a: &str| {
                Err::<$_type, utils::args::DeprecatedError>(
                    utils::args::DeprecatedError::RemovedArgument($message),
                )
            }
        };
    }

    /// Mark argument as deprecated with warning to be printed when it's used.
    #[macro_export]
    macro_rules! deprecated_arg {
        (bool, $message: expr) => {
            <_ as clap::builder::TypedValueParser>::map(
                clap::builder::BoolValueParser::new(),
                |x| {
                    if (x) {
                        $crate::print::Print::new(false).warnln($message);
                    }
                    x
                },
            )
        };
    }
}

pub mod rpc {
    use crate::xdr;
    use soroban_rpc::{Client, Error};
    use stellar_xdr::curr::{Hash, LedgerEntryData, LedgerKey, Limits, ReadXdr};

    pub async fn get_remote_wasm_from_hash(client: &Client, hash: &Hash) -> Result<Vec<u8>, Error> {
        let code_key = LedgerKey::ContractCode(xdr::LedgerKeyContractCode { hash: hash.clone() });
        let contract_data = client.get_ledger_entries(&[code_key]).await?;
        let entries = contract_data.entries.unwrap_or_default();
        if entries.is_empty() {
            return Err(Error::NotFound(
                "Contract Code".to_string(),
                hex::encode(hash),
            ));
        }
        let contract_data_entry = &entries[0];
        match LedgerEntryData::from_xdr_base64(&contract_data_entry.xdr, Limits::none())? {
            LedgerEntryData::ContractCode(xdr::ContractCodeEntry { code, .. }) => Ok(code.into()),
            scval => Err(Error::UnexpectedContractCodeDataType(scval)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contract_id_from_str() {
        // strkey
        match contract_id_from_str("CA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAXE") {
            Ok(contract_id) => assert_eq!(
                contract_id.0,
                [
                    0x36, 0x3e, 0xaa, 0x38, 0x67, 0x84, 0x1f, 0xba, 0xd0, 0xf4, 0xed, 0x88, 0xc7,
                    0x79, 0xe4, 0xfe, 0x66, 0xe5, 0x6a, 0x24, 0x70, 0xdc, 0x98, 0xc0, 0xec, 0x9c,
                    0x07, 0x3d, 0x05, 0xc7, 0xb1, 0x03,
                ]
            ),
            Err(err) => panic!("Failed to parse contract id: {err}"),
        }
    }
}
