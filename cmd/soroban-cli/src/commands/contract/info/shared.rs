use std::path::PathBuf;

use clap::arg;

use crate::{
    commands::contract::info::shared::Error::InvalidWasmHash,
    config::{
        self, locator,
        network::{self, Network},
    },
    print::Print,
    utils::rpc::get_remote_wasm_from_hash,
    wasm::{self, Error::ContractIsStellarAsset},
    xdr,
};

#[derive(Debug, clap::Args, Clone, Default)]
#[command(group(
    clap::ArgGroup::new("Source")
    .required(true)
    .args(& ["wasm", "wasm_hash", "contract_id"]),
))]
#[group(skip)]
pub struct Args {
    /// Wasm file path on local filesystem. Provide this OR `--wasm-hash` OR `--contract-id`.
    #[arg(
        long,
        group = "Source",
        conflicts_with = "contract_id",
        conflicts_with = "wasm_hash"
    )]
    pub wasm: Option<PathBuf>,
    /// Hash of Wasm blob on a network. Provide this OR `--wasm` OR `--contract-id`.
    #[arg(
        long = "wasm-hash",
        group = "Source",
        conflicts_with = "contract_id",
        conflicts_with = "wasm"
    )]
    pub wasm_hash: Option<String>,
    /// Contract ID/alias on a network. Provide this OR `--wasm-hash` OR `--wasm`.
    #[arg(
        long,
        env = "STELLAR_CONTRACT_ID",
        group = "Source",
        visible_alias = "id",
        conflicts_with = "wasm",
        conflicts_with = "wasm_hash"
    )]
    pub contract_id: Option<config::UnresolvedContract>,
    #[command(flatten)]
    pub network: network::Args,
    #[command(flatten)]
    pub locator: locator::Args,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, clap::ValueEnum, Default)]
pub enum MetasInfoOutput {
    /// Text output of the meta info entry
    #[default]
    Text,
    /// XDR output of the info entry
    XdrBase64,
    /// JSON output of the info entry (one line, not formatted)
    Json,
    /// Formatted (multiline) JSON output of the info entry
    JsonFormatted,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Wasm(#[from] wasm::Error),
    #[error("provided wasm hash is invalid {0:?}")]
    InvalidWasmHash(String),
    #[error("must provide one of --wasm, --wasm-hash, or --contract-id")]
    MissingArg,
    #[error(transparent)]
    Rpc(#[from] soroban_rpc::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
}

pub struct Fetched {
    pub contract: Contract,
    pub source: Source,
}

pub enum Contract {
    Wasm { wasm_bytes: Vec<u8> },
    StellarAssetContract,
}

pub enum Source {
    File {
        path: PathBuf,
    },
    Wasm {
        hash: String,
        network: Network,
    },
    Contract {
        resolved_address: String,
        network: Network,
    },
}

impl Source {
    pub fn network(&self) -> Option<&Network> {
        match self {
            Source::File { .. } => None,
            Source::Wasm { ref network, .. } | Source::Contract { ref network, .. } => {
                Some(network)
            }
        }
    }
}

pub async fn fetch(args: &Args, print: &Print) -> Result<Fetched, Error> {
    // Check if a local WASM file path is provided
    if let Some(path) = &args.wasm {
        // Read the WASM file and return its contents
        print.infoln("Loading contract spec from file...");
        let wasm_bytes = wasm::Args { wasm: path.clone() }.read()?;
        return Ok(Fetched {
            contract: Contract::Wasm { wasm_bytes },
            source: Source::File { path: path.clone() },
        });
    }

    // If no local wasm, then check for wasm_hash and fetch from the network
    let network = &args.network.get(&args.locator)?;
    print.infoln(format!("Network: {}", network.network_passphrase));

    if let Some(wasm_hash) = &args.wasm_hash {
        let hash = hex::decode(wasm_hash)
            .map_err(|_| InvalidWasmHash(wasm_hash.clone()))?
            .try_into()
            .map_err(|_| InvalidWasmHash(wasm_hash.clone()))?;

        let hash = xdr::Hash(hash);

        let client = network.rpc_client()?;

        client
            .verify_network_passphrase(Some(&network.network_passphrase))
            .await?;

        print.globeln(format!(
            "Downloading contract spec for wasm hash: {wasm_hash}"
        ));
        let wasm_bytes = get_remote_wasm_from_hash(&client, &hash).await?;
        Ok(Fetched {
            contract: Contract::Wasm { wasm_bytes },
            source: Source::Wasm {
                hash: wasm_hash.clone(),
                network: network.clone(),
            },
        })
    } else if let Some(contract_id) = &args.contract_id {
        let contract_id =
            contract_id.resolve_contract_id(&args.locator, &network.network_passphrase)?;
        let derived_address = xdr::ScAddress::Contract(xdr::Hash(contract_id.0)).to_string();
        print.globeln(format!("Downloading contract spec: {derived_address}"));
        let res = wasm::fetch_from_contract(&contract_id, network).await;
        if let Some(ContractIsStellarAsset) = res.as_ref().err() {
            return Ok(Fetched {
                contract: Contract::StellarAssetContract,
                source: Source::Contract {
                    resolved_address: derived_address,
                    network: network.clone(),
                },
            });
        }
        Ok(Fetched {
            contract: Contract::Wasm { wasm_bytes: res? },
            source: Source::Contract {
                resolved_address: derived_address,
                network: network.clone(),
            },
        })
    } else {
        return Err(Error::MissingArg);
    }
}
