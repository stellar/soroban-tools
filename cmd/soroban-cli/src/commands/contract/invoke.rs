use std::convert::{Infallible, TryInto};
use std::ffi::OsString;
use std::num::ParseIntError;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::{fmt::Debug, fs, io};

use clap::{arg, command, Parser, ValueEnum};

use soroban_rpc::{Client, SimulateHostFunctionResult, SimulateTransactionResponse};
use soroban_spec::read::FromWasmError;

use super::super::events;
use super::arg_parsing;
use crate::assembled::Assembled;
use crate::{
    assembled::simulate_and_assemble_transaction,
    commands::{
        contract::arg_parsing::{build_host_function_parameters, output_to_string},
        global,
        txn_result::{TxnEnvelopeResult, TxnResult},
        NetworkRunnable,
    },
    config::{self, data, locator, network},
    get_spec::{self, get_remote_contract_spec},
    print, rpc,
    xdr::{
        self, AccountEntry, AccountEntryExt, AccountId, ContractEvent, ContractEventType,
        DiagnosticEvent, HostFunction, InvokeContractArgs, InvokeHostFunctionOp, Limits, Memo,
        MuxedAccount, Operation, OperationBody, Preconditions, PublicKey, ScSpecEntry,
        SequenceNumber, String32, StringM, Thresholds, Transaction, TransactionExt, Uint256, VecM,
        WriteXdr,
    },
    Pwd,
};
use soroban_spec_tools::contract;

#[derive(Parser, Debug, Default, Clone)]
#[allow(clippy::struct_excessive_bools)]
#[group(skip)]
pub struct Cmd {
    /// Contract ID to invoke
    #[arg(long = "id", env = "STELLAR_CONTRACT_ID")]
    pub contract_id: config::UnresolvedContract,
    // For testing only
    #[arg(skip)]
    pub wasm: Option<std::path::PathBuf>,
    /// View the result simulating and do not sign and submit transaction. Deprecated use `--send=no`
    #[arg(long, env = "STELLAR_INVOKE_VIEW")]
    pub is_view: bool,
    /// Function name as subcommand, then arguments for that function as `--arg-name value`
    #[arg(last = true, id = "CONTRACT_FN_AND_ARGS")]
    pub slop: Vec<OsString>,
    #[command(flatten)]
    pub config: config::Args,
    #[command(flatten)]
    pub fee: crate::fee::Args,
    /// Whether or not to send a transaction
    #[arg(long, value_enum, default_value_t, env = "STELLAR_SEND")]
    pub send: Send,
}

impl FromStr for Cmd {
    type Err = clap::error::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use clap::{CommandFactory, FromArgMatches};
        Self::from_arg_matches_mut(&mut Self::command().get_matches_from(s.split_whitespace()))
    }
}

impl Pwd for Cmd {
    fn set_pwd(&mut self, pwd: &Path) {
        self.config.set_pwd(pwd);
    }
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("cannot add contract to ledger entries: {0}")]
    CannotAddContractToLedgerEntries(xdr::Error),
    #[error("reading file {0:?}: {1}")]
    CannotReadContractFile(PathBuf, io::Error),
    #[error("committing file {filepath}: {error}")]
    CannotCommitEventsFile {
        filepath: std::path::PathBuf,
        error: events::Error,
    },
    #[error("parsing contract spec: {0}")]
    CannotParseContractSpec(FromWasmError),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error("error parsing int: {0}")]
    ParseIntError(#[from] ParseIntError),
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error("missing operation result")]
    MissingOperationResult,
    #[error("error loading signing key: {0}")]
    SignatureError(#[from] ed25519_dalek::SignatureError),
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error("unexpected ({length}) simulate transaction result length")]
    UnexpectedSimulateTransactionResultSize { length: usize },
    #[error(transparent)]
    Clap(#[from] clap::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error("Contract Error\n{0}: {1}")]
    ContractInvoke(String, String),
    #[error(transparent)]
    StrKey(#[from] stellar_strkey::DecodeError),
    #[error(transparent)]
    ContractSpec(#[from] contract::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Data(#[from] data::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    GetSpecError(#[from] get_spec::Error),
    #[error(transparent)]
    ArgParsing(#[from] arg_parsing::Error),
}

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let res = self.invoke(global_args).await?.to_envelope();
        match res {
            TxnEnvelopeResult::TxnEnvelope(tx) => println!("{}", tx.to_xdr_base64(Limits::none())?),
            TxnEnvelopeResult::Res(output) => {
                println!("{output}");
            }
        }
        Ok(())
    }

    pub async fn invoke(&self, global_args: &global::Args) -> Result<TxnResult<String>, Error> {
        self.run_against_rpc_server(Some(global_args), None).await
    }

    pub fn read_wasm(&self) -> Result<Option<Vec<u8>>, Error> {
        Ok(if let Some(wasm) = self.wasm.as_ref() {
            Some(fs::read(wasm).map_err(|e| Error::CannotReadContractFile(wasm.clone(), e))?)
        } else {
            None
        })
    }

    pub fn spec_entries(&self) -> Result<Option<Vec<ScSpecEntry>>, Error> {
        self.read_wasm()?
            .map(|wasm| {
                soroban_spec::read::from_wasm(&wasm).map_err(Error::CannotParseContractSpec)
            })
            .transpose()
    }

    fn should_send_tx(&self, sim_res: &SimulateTransactionResponse) -> Result<ShouldSend, Error> {
        Ok(match self.send {
            Send::Default => {
                if self.is_view {
                    ShouldSend::No
                } else if has_write(sim_res)? || has_published_event(sim_res)? || has_auth(sim_res)?
                {
                    ShouldSend::Yes
                } else {
                    ShouldSend::DefaultNo
                }
            }
            Send::No => ShouldSend::No,
            Send::Yes => ShouldSend::Yes,
        })
    }

    // uses a default account to check if the tx should be sent after the simulation
    async fn simulate(
        &self,
        host_function_params: &InvokeContractArgs,
        account_details: &AccountEntry,
        rpc_client: &Client,
    ) -> Result<Assembled, Error> {
        let sequence: i64 = account_details.seq_num.0;
        let AccountId(PublicKey::PublicKeyTypeEd25519(account_id)) =
            account_details.account_id.clone();

        let tx = build_invoke_contract_tx(
            host_function_params.clone(),
            sequence + 1,
            self.fee.fee,
            account_id,
        )?;
        Ok(simulate_and_assemble_transaction(rpc_client, &tx).await?)
    }
}

#[async_trait::async_trait]
impl NetworkRunnable for Cmd {
    type Error = Error;
    type Result = TxnResult<String>;

    async fn run_against_rpc_server(
        &self,
        global_args: Option<&global::Args>,
        config: Option<&config::Args>,
    ) -> Result<TxnResult<String>, Error> {
        let config = config.unwrap_or(&self.config);
        let print = print::Print::new(global_args.map_or(false, |g| g.quiet));
        let network = config.get_network()?;
        tracing::trace!(?network);
        let contract_id = self
            .contract_id
            .resolve_contract_id(&config.locator, &network.network_passphrase)?;

        let spec_entries = self.spec_entries()?;
        if let Some(spec_entries) = &spec_entries {
            // For testing wasm arg parsing
            let _ = build_host_function_parameters(&contract_id, &self.slop, spec_entries, config)?;
        }
        let client = network.rpc_client()?;

        let spec_entries = get_remote_contract_spec(
            &contract_id.0,
            &config.locator,
            &config.network,
            global_args,
            Some(config),
        )
        .await
        .map_err(Error::from)?;

        let (function, spec, host_function_params, signers) =
            build_host_function_parameters(&contract_id, &self.slop, &spec_entries, config)?;

        let assembled = self
            .simulate(&host_function_params, &default_account_entry(), &client)
            .await?;
        let should_send = self.should_send_tx(&assembled.sim_res)?;

        let account_details = if should_send == ShouldSend::Yes {
            client
                .verify_network_passphrase(Some(&network.network_passphrase))
                .await?;

            client
                .get_account(&config.source_account()?.to_string())
                .await?
        } else {
            if should_send == ShouldSend::DefaultNo {
                print.infoln(
                    "Simulation identified as read-only. Send by rerunning with `--send=yes`.",
                );
            }
            let sim_res = assembled.sim_response();
            let (return_value, events) = (sim_res.results()?, sim_res.events()?);
            crate::log::event::all(&events);
            crate::log::event::contract(&events, &print);
            return Ok(output_to_string(&spec, &return_value[0].xdr, &function)?);
        };
        let sequence: i64 = account_details.seq_num.into();
        let AccountId(PublicKey::PublicKeyTypeEd25519(account_id)) = account_details.account_id;

        let tx = Box::new(build_invoke_contract_tx(
            host_function_params.clone(),
            sequence + 1,
            self.fee.fee,
            account_id,
        )?);
        if self.fee.build_only {
            return Ok(TxnResult::Txn(tx));
        }
        let txn = simulate_and_assemble_transaction(&client, &tx).await?;
        let assembled = self.fee.apply_to_assembled_txn(txn);
        let mut txn = Box::new(assembled.transaction().clone());
        if self.fee.sim_only {
            return Ok(TxnResult::Txn(txn));
        }
        let sim_res = assembled.sim_response();
        if global_args.map_or(true, |a| !a.no_cache) {
            data::write(sim_res.clone().into(), &network.rpc_uri()?)?;
        }
        let global::Args { no_cache, .. } = global_args.cloned().unwrap_or_default();
        // Need to sign all auth entries
        if let Some(tx) = config.sign_soroban_authorizations(&txn, &signers).await? {
            txn = Box::new(tx);
        }
        let res = client
            .send_transaction_polling(&config.sign_with_local_key(*txn).await?)
            .await?;
        if !no_cache {
            data::write(res.clone().try_into()?, &network.rpc_uri()?)?;
        }
        let events = res
            .result_meta
            .as_ref()
            .map(crate::log::extract_events)
            .unwrap_or_default();
        let return_value = res.return_value()?;

        crate::log::event::all(&events);
        crate::log::event::contract(&events, &print);
        Ok(output_to_string(&spec, &return_value, &function)?)
    }
}

const DEFAULT_ACCOUNT_ID: AccountId = AccountId(PublicKey::PublicKeyTypeEd25519(Uint256([0; 32])));

fn default_account_entry() -> AccountEntry {
    AccountEntry {
        account_id: DEFAULT_ACCOUNT_ID,
        balance: 0,
        seq_num: SequenceNumber(0),
        num_sub_entries: 0,
        inflation_dest: None,
        flags: 0,
        home_domain: String32::from(unsafe { StringM::<32>::from_str("TEST").unwrap_unchecked() }),
        thresholds: Thresholds([0; 4]),
        signers: unsafe { [].try_into().unwrap_unchecked() },
        ext: AccountEntryExt::V0,
    }
}

fn build_invoke_contract_tx(
    parameters: InvokeContractArgs,
    sequence: i64,
    fee: u32,
    source_account_id: Uint256,
) -> Result<Transaction, Error> {
    let op = Operation {
        source_account: None,
        body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
            host_function: HostFunction::InvokeContract(parameters),
            auth: VecM::default(),
        }),
    };
    Ok(Transaction {
        source_account: MuxedAccount::Ed25519(source_account_id),
        fee,
        seq_num: SequenceNumber(sequence),
        cond: Preconditions::None,
        memo: Memo::None,
        operations: vec![op].try_into()?,
        ext: TransactionExt::V0,
    })
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ValueEnum, Default)]
pub enum Send {
    /// Send transaction if simulation indicates there are ledger writes,
    /// published events, or auth required, otherwise return simulation result
    #[default]
    Default,
    /// Do not send transaction, return simulation result
    No,
    /// Always send transaction
    Yes,
}

#[derive(Debug, PartialEq)]
enum ShouldSend {
    DefaultNo,
    No,
    Yes,
}

fn has_write(sim_res: &SimulateTransactionResponse) -> Result<bool, Error> {
    Ok(!sim_res
        .transaction_data()?
        .resources
        .footprint
        .read_write
        .is_empty())
}

fn has_published_event(sim_res: &SimulateTransactionResponse) -> Result<bool, Error> {
    Ok(sim_res.events()?.iter().any(
        |DiagnosticEvent {
             event: ContractEvent { type_, .. },
             ..
         }| matches!(type_, ContractEventType::Contract),
    ))
}

fn has_auth(sim_res: &SimulateTransactionResponse) -> Result<bool, Error> {
    Ok(sim_res
        .results()?
        .iter()
        .any(|SimulateHostFunctionResult { auth, .. }| !auth.is_empty()))
}
