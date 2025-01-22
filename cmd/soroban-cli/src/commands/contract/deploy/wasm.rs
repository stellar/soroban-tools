use crate::commands::contract::deploy::utils::alias_validator;
use std::array::TryFromSliceError;
use std::ffi::OsString;
use std::fmt::Debug;
use std::num::ParseIntError;

use crate::xdr::{
    AccountId, ContractExecutable, ContractIdPreimage, ContractIdPreimageFromAddress,
    CreateContractArgs, CreateContractArgsV2, Error as XdrError, Hash, HostFunction,
    InvokeContractArgs, InvokeHostFunctionOp, Limits, Memo, MuxedAccount, Operation, OperationBody,
    Preconditions, PublicKey, ScAddress, SequenceNumber, Transaction, TransactionExt, Uint256,
    VecM, WriteXdr,
};
use clap::{arg, command, Parser};
use rand::Rng;

use soroban_spec_tools::contract as contract_spec;

use crate::{
    assembled::simulate_and_assemble_transaction,
    commands::{
        contract::{self, arg_parsing, id::wasm::get_contract_id, install},
        global,
        txn_result::{TxnEnvelopeResult, TxnResult},
        NetworkRunnable, HEADING_RPC,
    },
    config::{self, data, locator, network},
    print::Print,
    rpc,
    utils::{self, rpc::get_remote_wasm_from_hash},
    wasm,
};

pub const CONSTRUCTOR_FUNCTION_NAME: &str = "__constructor";

#[derive(Parser, Debug, Clone)]
#[command(group(
    clap::ArgGroup::new("wasm_src")
        .required(true)
        .args(&["wasm", "wasm_hash"]),
))]
#[group(skip)]
pub struct Cmd {
    /// WASM file to deploy
    #[arg(long, group = "wasm_src")]
    pub wasm: Option<std::path::PathBuf>,
    /// Hash of the already installed/deployed WASM file
    #[arg(long = "wasm-hash", conflicts_with = "wasm", group = "wasm_src")]
    pub wasm_hash: Option<String>,
    /// Custom salt 32-byte salt for the token id
    #[arg(
        long,
        help_heading = HEADING_RPC,
    )]
    pub salt: Option<String>,
    #[command(flatten)]
    pub config: config::Args,
    #[command(flatten)]
    pub fee: crate::fee::Args,
    #[arg(long, short = 'i', default_value = "false")]
    /// Whether to ignore safety checks when deploying contracts
    pub ignore_checks: bool,
    /// The alias that will be used to save the contract's id.
    /// Whenever used, `--alias` will always overwrite the existing contract id
    /// configuration without asking for confirmation.
    #[arg(long, value_parser = clap::builder::ValueParser::new(alias_validator))]
    pub alias: Option<String>,
    /// If provided, will be passed to the contract's `__constructor` function with provided arguments for that function as `--arg-name value`
    #[arg(last = true, id = "CONTRACT_CONSTRUCTOR_ARGS")]
    pub slop: Vec<OsString>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Install(#[from] install::Error),
    #[error("error parsing int: {0}")]
    ParseIntError(#[from] ParseIntError),
    #[error("internal conversion error: {0}")]
    TryFromSliceError(#[from] TryFromSliceError),
    #[error("xdr processing error: {0}")]
    Xdr(#[from] XdrError),
    #[error("jsonrpc error: {0}")]
    JsonRpc(#[from] jsonrpsee_core::Error),
    #[error("cannot parse salt: {salt}")]
    CannotParseSalt { salt: String },
    #[error("cannot parse contract ID {contract_id}: {error}")]
    CannotParseContractId {
        contract_id: String,
        error: stellar_strkey::DecodeError,
    },
    #[error("cannot parse WASM hash {wasm_hash}: {error}")]
    CannotParseWasmHash {
        wasm_hash: String,
        error: stellar_strkey::DecodeError,
    },
    #[error("Must provide either --wasm or --wash-hash")]
    WasmNotProvided,
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error(transparent)]
    StrKey(#[from] stellar_strkey::DecodeError),
    #[error(transparent)]
    Infallible(#[from] std::convert::Infallible),
    #[error(transparent)]
    WasmId(#[from] contract::id::wasm::Error),
    #[error(transparent)]
    Data(#[from] data::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Wasm(#[from] wasm::Error),
    #[error(
        "alias must be 1-30 chars long, and have only letters, numbers, underscores and dashes"
    )]
    InvalidAliasFormat { alias: String },
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    ContractSpec(#[from] contract_spec::Error),
    #[error(transparent)]
    ArgParse(#[from] arg_parsing::Error),
    #[error("Only ed25519 accounts are allowed")]
    OnlyEd25519AccountsAllowed,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let res = self
            .run_against_rpc_server(Some(global_args), None)
            .await?
            .to_envelope();
        match res {
            TxnEnvelopeResult::TxnEnvelope(tx) => println!("{}", tx.to_xdr_base64(Limits::none())?),
            TxnEnvelopeResult::Res(contract) => {
                let network = self.config.get_network()?;

                if let Some(alias) = self.alias.clone() {
                    if let Some(existing_contract) = self
                        .config
                        .locator
                        .get_contract_id(&alias, &network.network_passphrase)?
                    {
                        let print = Print::new(global_args.quiet);
                        print.warnln(format!(
                            "Overwriting existing contract id: {existing_contract}"
                        ));
                    };

                    self.config.locator.save_contract_id(
                        &network.network_passphrase,
                        &contract,
                        &alias,
                    )?;
                }

                println!("{contract}");
            }
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl NetworkRunnable for Cmd {
    type Error = Error;
    type Result = TxnResult<stellar_strkey::Contract>;

    #[allow(clippy::too_many_lines)]
    async fn run_against_rpc_server(
        &self,
        global_args: Option<&global::Args>,
        config: Option<&config::Args>,
    ) -> Result<TxnResult<stellar_strkey::Contract>, Error> {
        let print = Print::new(global_args.map_or(false, |a| a.quiet));
        let config = config.unwrap_or(&self.config);
        let wasm_hash = if let Some(wasm) = &self.wasm {
            let hash = if self.fee.build_only || self.fee.sim_only {
                wasm::Args { wasm: wasm.clone() }.hash()?
            } else {
                install::Cmd {
                    wasm: wasm::Args { wasm: wasm.clone() },
                    config: config.clone(),
                    fee: self.fee.clone(),
                    ignore_checks: self.ignore_checks,
                }
                .run_against_rpc_server(global_args, Some(config))
                .await?
                .into_result()
                .expect("the value (hash) is expected because it should always be available since build-only is a shared parameter")
            };
            hex::encode(hash)
        } else {
            self.wasm_hash
                .as_ref()
                .ok_or(Error::WasmNotProvided)?
                .to_string()
        };

        let wasm_hash = Hash(
            utils::contract_id_from_str(&wasm_hash)
                .map_err(|e| Error::CannotParseWasmHash {
                    wasm_hash: wasm_hash.clone(),
                    error: e,
                })?
                .0,
        );

        print.infoln(format!("Using wasm hash {wasm_hash}").as_str());

        let network = config.get_network()?;
        let salt: [u8; 32] = match &self.salt {
            Some(h) => soroban_spec_tools::utils::padded_hex_from_str(h, 32)
                .map_err(|_| Error::CannotParseSalt { salt: h.clone() })?
                .try_into()
                .map_err(|_| Error::CannotParseSalt { salt: h.clone() })?,
            None => rand::thread_rng().gen::<[u8; 32]>(),
        };

        let client = network.rpc_client()?;
        client
            .verify_network_passphrase(Some(&network.network_passphrase))
            .await?;

        let MuxedAccount::Ed25519(bytes) = config.source_account()? else {
            return Err(Error::OnlyEd25519AccountsAllowed);
        };
        let source_account = AccountId(PublicKey::PublicKeyTypeEd25519(bytes));
        let contract_id_preimage = ContractIdPreimage::Address(ContractIdPreimageFromAddress {
            address: ScAddress::Account(source_account.clone()),
            salt: Uint256(salt),
        });
        let contract_id =
            get_contract_id(contract_id_preimage.clone(), &network.network_passphrase)?;
        let raw_wasm = if let Some(wasm) = self.wasm.as_ref() {
            wasm::Args { wasm: wasm.clone() }.read()?
        } else {
            get_remote_wasm_from_hash(&client, &wasm_hash).await?
        };
        let entries = soroban_spec_tools::contract::Spec::new(&raw_wasm)?.spec;
        let res = soroban_spec_tools::Spec::new(entries.clone());
        let constructor_params = if let Ok(func) = res.find_function(CONSTRUCTOR_FUNCTION_NAME) {
            if func.inputs.len() == 0 {
                None
            } else {
                let mut slop = vec![OsString::from(CONSTRUCTOR_FUNCTION_NAME)];
                slop.extend_from_slice(&self.slop);
                Some(
                    arg_parsing::build_host_function_parameters(
                        &stellar_strkey::Contract(contract_id.0),
                        &slop,
                        &entries,
                        config,
                    )?
                    .2,
                )
            }
        } else {
            None
        };

        // Get the account sequence number
        let account_details = client.get_account(&source_account.to_string()).await?;
        let sequence: i64 = account_details.seq_num.into();
        let txn = Box::new(build_create_contract_tx(
            wasm_hash,
            sequence + 1,
            self.fee.fee,
            source_account,
            contract_id_preimage,
            constructor_params.as_ref(),
        )?);

        if self.fee.build_only {
            print.checkln("Transaction built!");
            return Ok(TxnResult::Txn(txn));
        }

        print.infoln("Simulating deploy transaction…");

        let txn = simulate_and_assemble_transaction(&client, &txn).await?;
        let txn = Box::new(self.fee.apply_to_assembled_txn(txn).transaction().clone());

        if self.fee.sim_only {
            print.checkln("Done!");
            return Ok(TxnResult::Txn(txn));
        }

        print.globeln("Submitting deploy transaction…");
        print.log_transaction(&txn, &network, true)?;

        let get_txn_resp = client
            .send_transaction_polling(&config.sign_with_local_key(*txn).await?)
            .await?
            .try_into()?;

        if global_args.map_or(true, |a| !a.no_cache) {
            data::write(get_txn_resp, &network.rpc_uri()?)?;
        }

        if let Some(url) = utils::explorer_url_for_contract(&network, &contract_id) {
            print.linkln(url);
        }

        print.checkln("Deployed!");

        Ok(TxnResult::Res(contract_id))
    }
}

fn build_create_contract_tx(
    wasm_hash: Hash,
    sequence: i64,
    fee: u32,
    key: AccountId,
    contract_id_preimage: ContractIdPreimage,
    constructor_params: Option<&InvokeContractArgs>,
) -> Result<Transaction, Error> {
    let op = if let Some(InvokeContractArgs { args, .. }) = constructor_params {
        Operation {
            source_account: None,
            body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
                host_function: HostFunction::CreateContractV2(CreateContractArgsV2 {
                    contract_id_preimage,
                    executable: ContractExecutable::Wasm(wasm_hash),
                    constructor_args: args.clone(),
                }),
                auth: VecM::default(),
            }),
        }
    } else {
        Operation {
            source_account: None,
            body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
                host_function: HostFunction::CreateContract(CreateContractArgs {
                    contract_id_preimage,
                    executable: ContractExecutable::Wasm(wasm_hash),
                }),
                auth: VecM::default(),
            }),
        }
    };
    let tx = Transaction {
        source_account: key.into(),
        fee,
        seq_num: SequenceNumber(sequence),
        cond: Preconditions::None,
        memo: Memo::None,
        operations: vec![op].try_into()?,
        ext: TransactionExt::V0,
    };

    Ok(tx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_create_contract() {
        let hash = hex::decode("0000000000000000000000000000000000000000000000000000000000000000")
            .unwrap()
            .try_into()
            .unwrap();
        let salt = [0u8; 32];
        let key =
            &utils::parse_secret_key("SBFGFF27Y64ZUGFAIG5AMJGQODZZKV2YQKAVUUN4HNE24XZXD2OEUVUP")
                .unwrap();
        let source_account = AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(
            key.verifying_key().to_bytes(),
        )));

        let contract_id_preimage = ContractIdPreimage::Address(ContractIdPreimageFromAddress {
            address: ScAddress::Account(source_account.clone()),
            salt: Uint256(salt),
        });

        let result = build_create_contract_tx(
            Hash(hash),
            300,
            1,
            source_account,
            contract_id_preimage,
            None,
        );

        assert!(result.is_ok());
    }
}
