use crate::xdr;

use crate::xdr::{ContractDataEntry, ContractExecutable, ScContractInstance, ScSpecEntry, ScVal};

use soroban_spec::read::FromWasmError;
pub use soroban_spec_tools::contract as contract_spec;

use crate::commands::global;
use crate::config::{self, data, locator, network};
use crate::rpc;
use crate::utils::rpc::get_remote_wasm_from_hash;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("parsing contract spec: {0}")]
    CannotParseContractSpec(FromWasmError),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error("missing result")]
    MissingResult,
    #[error(transparent)]
    Data(#[from] data::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error(transparent)]
    ContractSpec(#[from] contract_spec::Error),
}

///
/// # Errors
pub async fn get_remote_contract_spec(
    contract_id: &[u8; 32],
    locator: &locator::Args,
    network: &network::Args,
    global_args: Option<&global::Args>,
    config: Option<&config::Args>,
) -> Result<Vec<ScSpecEntry>, Error> {
    let network = config.map_or_else(
        || network.get(locator).map_err(Error::from),
        |c| c.get_network().map_err(Error::from),
    )?;
    tracing::trace!(?network);
    let client = network.rpc_client()?;
    // Get contract data
    let r = client.get_contract_data(contract_id).await?;
    tracing::trace!("{r:?}");

    let ContractDataEntry {
        val: ScVal::ContractInstance(ScContractInstance { executable, .. }),
        ..
    } = r
    else {
        return Err(Error::MissingResult);
    };

    // Get the contract spec entries based on the executable type
    Ok(match executable {
        ContractExecutable::Wasm(hash) => {
            let hash_str = hash.to_string();
            if let Ok(entries) = data::read_spec(&hash_str) {
                entries
            } else {
                let raw_wasm = get_remote_wasm_from_hash(&client, &hash).await?;
                let res = contract_spec::Spec::new(&raw_wasm)?;
                let res = res.spec;
                if global_args.map_or(true, |a| !a.no_cache) {
                    data::write_spec(&hash_str, &res)?;
                }
                res
            }
        }
        ContractExecutable::StellarAsset => {
            soroban_spec::read::parse_raw(&soroban_sdk::token::StellarAssetSpec::spec_xdr())?
        }
    })
}
