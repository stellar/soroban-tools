use crate::config::locator;
use crate::print::Print;
use crate::xdr::{
    Asset, ContractDataDurability, ContractExecutable, ContractIdPreimage, CreateContractArgs,
    Error as XdrError, Hash, HostFunction, InvokeHostFunctionOp, LedgerKey::ContractData,
    LedgerKeyContractData, Limits, Memo, MuxedAccount, Operation, OperationBody, Preconditions,
    ScAddress, ScVal, SequenceNumber, Transaction, TransactionExt, VecM, WriteXdr,
};
use clap::{arg, command, Parser};
use std::convert::Infallible;
use std::{array::TryFromSliceError, fmt::Debug, num::ParseIntError};

use crate::{
    assembled::simulate_and_assemble_transaction,
    commands::{
        global,
        txn_result::{TxnEnvelopeResult, TxnResult},
        NetworkRunnable,
    },
    config::{self, data, network},
    rpc::Error as SorobanRpcError,
    tx::builder,
    utils::contract_id_hash_from_asset,
};

use crate::commands::contract::deploy::utils::alias_validator;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("error parsing int: {0}")]
    ParseIntError(#[from] ParseIntError),
    #[error(transparent)]
    Client(#[from] SorobanRpcError),
    #[error("internal conversion error: {0}")]
    TryFromSliceError(#[from] TryFromSliceError),
    #[error("xdr processing error: {0}")]
    Xdr(#[from] XdrError),
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error(transparent)]
    Data(#[from] data::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Builder(#[from] builder::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
}

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// ID of the Stellar classic asset to wrap, e.g. "USDC:G...5"
    #[arg(long)]
    pub asset: builder::Asset,

    #[command(flatten)]
    pub config: config::Args,

    #[command(flatten)]
    pub fee: crate::fee::Args,

    /// The alias that will be used to save the assets's id.
    /// Whenever used, `--alias` will always overwrite the existing contract id
    /// configuration without asking for confirmation.
    #[arg(long, value_parser = clap::builder::ValueParser::new(alias_validator))]
    pub alias: Option<String>,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let res = self.run_against_rpc_server(None, None).await?.to_envelope();
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

    async fn run_against_rpc_server(
        &self,
        args: Option<&global::Args>,
        config: Option<&config::Args>,
    ) -> Result<Self::Result, Error> {
        let config = config.unwrap_or(&self.config);
        // Parse asset
        let asset = &self.asset;

        let network = config.get_network()?;
        let client = network.rpc_client()?;
        client
            .verify_network_passphrase(Some(&network.network_passphrase))
            .await?;
        let source_account = config.source_account()?;
        // Get the account sequence number
        // TODO: use symbols for the method names (both here and in serve)
        let account_details = client
            .get_account(&source_account.clone().to_string())
            .await?;
        let sequence: i64 = account_details.seq_num.into();
        let network_passphrase = &network.network_passphrase;
        let contract_id = contract_id_hash_from_asset(asset, network_passphrase);
        let tx = build_wrap_token_tx(
            asset,
            &contract_id,
            sequence + 1,
            self.fee.fee,
            network_passphrase,
            source_account,
        )?;
        if self.fee.build_only {
            return Ok(TxnResult::Txn(Box::new(tx)));
        }
        let txn = simulate_and_assemble_transaction(&client, &tx).await?;
        let txn = self.fee.apply_to_assembled_txn(txn).transaction().clone();
        if self.fee.sim_only {
            return Ok(TxnResult::Txn(Box::new(txn)));
        }
        let get_txn_resp = client
            .send_transaction_polling(&self.config.sign_with_local_key(txn).await?)
            .await?
            .try_into()?;
        if args.map_or(true, |a| !a.no_cache) {
            data::write(get_txn_resp, &network.rpc_uri()?)?;
        }

        Ok(TxnResult::Res(stellar_strkey::Contract(contract_id.0)))
    }
}

fn build_wrap_token_tx(
    asset: impl Into<Asset>,
    contract_id: &stellar_strkey::Contract,
    sequence: i64,
    fee: u32,
    _network_passphrase: &str,
    source_account: MuxedAccount,
) -> Result<Transaction, Error> {
    let contract = ScAddress::Contract(Hash(contract_id.0));
    let mut read_write = vec![
        ContractData(LedgerKeyContractData {
            contract: contract.clone(),
            key: ScVal::LedgerKeyContractInstance,
            durability: ContractDataDurability::Persistent,
        }),
        ContractData(LedgerKeyContractData {
            contract: contract.clone(),
            key: ScVal::Vec(Some(
                vec![ScVal::Symbol("Metadata".try_into().unwrap())].try_into()?,
            )),
            durability: ContractDataDurability::Persistent,
        }),
    ];
    let asset = asset.into();
    if asset != Asset::Native {
        read_write.push(ContractData(LedgerKeyContractData {
            contract,
            key: ScVal::Vec(Some(
                vec![ScVal::Symbol("Admin".try_into().unwrap())].try_into()?,
            )),
            durability: ContractDataDurability::Persistent,
        }));
    }

    let op = Operation {
        source_account: None,
        body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
            host_function: HostFunction::CreateContract(CreateContractArgs {
                contract_id_preimage: ContractIdPreimage::Asset(asset),
                executable: ContractExecutable::StellarAsset,
            }),
            auth: VecM::default(),
        }),
    };

    Ok(Transaction {
        source_account,
        fee,
        seq_num: SequenceNumber(sequence),
        cond: Preconditions::None,
        memo: Memo::None,
        operations: vec![op].try_into()?,
        ext: TransactionExt::V0,
    })
}
