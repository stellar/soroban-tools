use std::array::TryFromSliceError;
use std::fmt::Debug;
use std::num::ParseIntError;

use crate::xdr::{
    self, ContractCodeEntryExt, Error as XdrError, Hash, HostFunction, InvokeHostFunctionOp,
    LedgerEntryData, Limits, OperationBody, ReadXdr, ScMetaEntry, ScMetaV0, Transaction,
    TransactionResult, TransactionResultResult, VecM, WriteXdr,
};
use clap::{command, Parser};

use super::restore;
use crate::{
    assembled::simulate_and_assemble_transaction,
    commands::{
        global,
        txn_result::{TxnEnvelopeResult, TxnResult},
        NetworkRunnable,
    },
    config::{self, data, network},
    key,
    print::Print,
    rpc,
    tx::builder::{self, TxExt},
    utils, wasm,
};

const CONTRACT_META_SDK_KEY: &str = "rssdkver";
const PUBLIC_NETWORK_PASSPHRASE: &str = "Public Global Stellar Network ; September 2015";

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub config: config::Args,
    #[command(flatten)]
    pub fee: crate::fee::Args,
    #[command(flatten)]
    pub wasm: wasm::Args,
    #[arg(long, short = 'i', default_value = "false")]
    /// Whether to ignore safety checks when deploying contracts
    pub ignore_checks: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("error parsing int: {0}")]
    ParseIntError(#[from] ParseIntError),
    #[error("internal conversion error: {0}")]
    TryFromSliceError(#[from] TryFromSliceError),
    #[error("xdr processing error: {0}")]
    Xdr(#[from] XdrError),
    #[error("jsonrpc error: {0}")]
    JsonRpc(#[from] jsonrpsee_core::Error),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error(transparent)]
    Wasm(#[from] wasm::Error),
    #[error("unexpected ({length}) simulate transaction result length")]
    UnexpectedSimulateTransactionResultSize { length: usize },
    #[error(transparent)]
    Restore(#[from] restore::Error),
    #[error("cannot parse WASM file {wasm}: {error}")]
    CannotParseWasm {
        wasm: std::path::PathBuf,
        error: wasm::Error,
    },
    #[error("the deployed smart contract {wasm} was built with Soroban Rust SDK v{version}, a release candidate version not intended for use with the Stellar Public Network. To deploy anyway, use --ignore-checks")]
    ContractCompiledWithReleaseCandidateSdk {
        wasm: std::path::PathBuf,
        version: String,
    },
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Data(#[from] data::Error),
    #[error(transparent)]
    Builder(#[from] builder::Error),
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let res = self
            .run_against_rpc_server(Some(global_args), None)
            .await?
            .to_envelope();
        match res {
            TxnEnvelopeResult::TxnEnvelope(tx) => println!("{}", tx.to_xdr_base64(Limits::none())?),
            TxnEnvelopeResult::Res(hash) => println!("{}", hex::encode(hash)),
        };
        Ok(())
    }
}

#[async_trait::async_trait]
impl NetworkRunnable for Cmd {
    type Error = Error;
    type Result = TxnResult<Hash>;

    async fn run_against_rpc_server(
        &self,
        args: Option<&global::Args>,
        config: Option<&config::Args>,
    ) -> Result<TxnResult<Hash>, Error> {
        let print = Print::new(args.map_or(false, |a| a.quiet));
        let config = config.unwrap_or(&self.config);
        let contract = self.wasm.read()?;
        let network = config.get_network()?;
        let client = network.rpc_client()?;
        client
            .verify_network_passphrase(Some(&network.network_passphrase))
            .await?;
        let wasm_spec = &self.wasm.parse().map_err(|e| Error::CannotParseWasm {
            wasm: self.wasm.wasm.clone(),
            error: e,
        })?;

        // Check Rust SDK version if using the public network.
        if let Some(rs_sdk_ver) = get_contract_meta_sdk_version(wasm_spec) {
            if rs_sdk_ver.contains("rc")
                && !self.ignore_checks
                && network.network_passphrase == PUBLIC_NETWORK_PASSPHRASE
            {
                return Err(Error::ContractCompiledWithReleaseCandidateSdk {
                    wasm: self.wasm.wasm.clone(),
                    version: rs_sdk_ver,
                });
            } else if rs_sdk_ver.contains("rc")
                && network.network_passphrase == PUBLIC_NETWORK_PASSPHRASE
            {
                tracing::warn!("the deployed smart contract {path} was built with Soroban Rust SDK v{rs_sdk_ver}, a release candidate version not intended for use with the Stellar Public Network", path = self.wasm.wasm.display());
            }
        }

        // Get the account sequence number
        let source_account = config.source_account()?;

        let account_details = client
            .get_account(&source_account.clone().to_string())
            .await?;
        let sequence: i64 = account_details.seq_num.into();

        let (tx_without_preflight, hash) =
            build_install_contract_code_tx(&contract, sequence + 1, self.fee.fee, &source_account)?;

        if self.fee.build_only {
            return Ok(TxnResult::Txn(Box::new(tx_without_preflight)));
        }

        // Don't check whether the contract is already installed when the user
        // has requested to perform simulation only and is hoping to get a
        // transaction back.
        if !self.fee.sim_only {
            let code_key =
                xdr::LedgerKey::ContractCode(xdr::LedgerKeyContractCode { hash: hash.clone() });
            let contract_data = client.get_ledger_entries(&[code_key]).await?;

            // Skip install if the contract is already installed, and the contract has an extension version that isn't V0.
            // In protocol 21 extension V1 was added that stores additional information about a contract making execution
            // of the contract cheaper. So if folks want to reinstall we should let them which is why the install will still
            // go ahead if the contract has a V0 extension.
            if let Some(entries) = contract_data.entries {
                if let Some(entry_result) = entries.first() {
                    let entry: LedgerEntryData =
                        LedgerEntryData::from_xdr_base64(&entry_result.xdr, Limits::none())?;

                    match &entry {
                        LedgerEntryData::ContractCode(code) => {
                            // Skip reupload if this isn't V0 because V1 extension already
                            // exists.
                            if code.ext.ne(&ContractCodeEntryExt::V0) {
                                print.infoln("Skipping install because wasm already installed");
                                return Ok(TxnResult::Res(hash));
                            }
                        }
                        _ => {
                            tracing::warn!("Entry retrieved should be of type ContractCode");
                        }
                    }
                }
            }
        }

        print.infoln("Simulating install transaction…");

        let txn = simulate_and_assemble_transaction(&client, &tx_without_preflight).await?;
        let txn = Box::new(self.fee.apply_to_assembled_txn(txn).transaction().clone());

        if self.fee.sim_only {
            return Ok(TxnResult::Txn(txn));
        }

        let signed_txn = &self.config.sign_with_local_key(*txn).await?;

        print.globeln("Submitting install transaction…");
        let txn_resp = client.send_transaction_polling(signed_txn).await?;

        if args.map_or(true, |a| !a.no_cache) {
            data::write(txn_resp.clone().try_into().unwrap(), &network.rpc_uri()?)?;
        }

        // Currently internal errors are not returned if the contract code is expired
        if let Some(TransactionResult {
            result: TransactionResultResult::TxInternalError,
            ..
        }) = txn_resp.result
        {
            // Now just need to restore it and don't have to install again
            restore::Cmd {
                key: key::Args {
                    contract_id: None,
                    key: None,
                    key_xdr: None,
                    wasm: Some(self.wasm.wasm.clone()),
                    wasm_hash: None,
                    durability: super::Durability::Persistent,
                },
                config: config.clone(),
                fee: self.fee.clone(),
                ledgers_to_extend: None,
                ttl_ledger_only: true,
            }
            .run_against_rpc_server(args, None)
            .await?;
        }

        if args.map_or(true, |a| !a.no_cache) {
            data::write_spec(&hash.to_string(), &wasm_spec.spec)?;
        }

        Ok(TxnResult::Res(hash))
    }
}

fn get_contract_meta_sdk_version(wasm_spec: &soroban_spec_tools::contract::Spec) -> Option<String> {
    let rs_sdk_version_option = if let Some(_meta) = &wasm_spec.meta_base64 {
        wasm_spec.meta.iter().find(|entry| match entry {
            ScMetaEntry::ScMetaV0(ScMetaV0 { key, .. }) => {
                key.to_utf8_string_lossy().contains(CONTRACT_META_SDK_KEY)
            }
        })
    } else {
        None
    };

    if let Some(rs_sdk_version_entry) = &rs_sdk_version_option {
        match rs_sdk_version_entry {
            ScMetaEntry::ScMetaV0(ScMetaV0 { val, .. }) => {
                return Some(val.to_utf8_string_lossy());
            }
        }
    }

    None
}

pub(crate) fn build_install_contract_code_tx(
    source_code: &[u8],
    sequence: i64,
    fee: u32,
    source: &xdr::MuxedAccount,
) -> Result<(Transaction, Hash), Error> {
    let hash = utils::contract_hash(source_code)?;

    let op = xdr::Operation {
        source_account: None,
        body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
            host_function: HostFunction::UploadContractWasm(source_code.try_into()?),
            auth: VecM::default(),
        }),
    };
    let tx = Transaction::new_tx(source.clone(), fee, sequence, op);

    Ok((tx, hash))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_install_contract_code() {
        let result = build_install_contract_code_tx(
            b"foo",
            300,
            1,
            &stellar_strkey::ed25519::PublicKey::from_payload(
                utils::parse_secret_key("SBFGFF27Y64ZUGFAIG5AMJGQODZZKV2YQKAVUUN4HNE24XZXD2OEUVUP")
                    .unwrap()
                    .verifying_key()
                    .as_bytes(),
            )
            .unwrap()
            .to_string()
            .parse()
            .unwrap(),
        );

        assert!(result.is_ok());
    }
}
