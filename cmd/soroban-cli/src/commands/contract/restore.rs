use std::{fmt::Debug, path::Path, str::FromStr};

use crate::xdr::{
    Error as XdrError, ExtensionPoint, LedgerEntry, LedgerEntryChange, LedgerEntryData,
    LedgerFootprint, Limits, Memo, Operation, OperationBody, OperationMeta, Preconditions,
    RestoreFootprintOp, SequenceNumber, SorobanResources, SorobanTransactionData, Transaction,
    TransactionExt, TransactionMeta, TransactionMetaV3, TtlEntry, WriteXdr,
};
use clap::{command, Parser};
use stellar_strkey::DecodeError;

use crate::{
    commands::{
        contract::extend,
        global,
        txn_result::{TxnEnvelopeResult, TxnResult},
        NetworkRunnable,
    },
    config::{self, data, locator, network},
    key, rpc, wasm, Pwd,
};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub key: key::Args,
    /// Number of ledgers to extend the entry
    #[arg(long)]
    pub ledgers_to_extend: Option<u32>,
    /// Only print the new Time To Live ledger
    #[arg(long)]
    pub ttl_ledger_only: bool,
    #[command(flatten)]
    pub config: config::Args,
    #[command(flatten)]
    pub fee: crate::fee::Args,
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
    #[error("parsing key {key}: {error}")]
    CannotParseKey {
        key: String,
        error: soroban_spec_tools::Error,
    },
    #[error("parsing XDR key {key}: {error}")]
    CannotParseXdrKey { key: String, error: XdrError },
    #[error("cannot parse contract ID {0}: {1}")]
    CannotParseContractId(String, DecodeError),
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error("either `--key` or `--key-xdr` are required")]
    KeyIsRequired,
    #[error("xdr processing error: {0}")]
    Xdr(#[from] XdrError),
    #[error("Ledger entry not found")]
    LedgerEntryNotFound,
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error("missing operation result")]
    MissingOperationResult,
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Wasm(#[from] wasm::Error),
    #[error(transparent)]
    Key(#[from] key::Error),
    #[error(transparent)]
    Extend(#[from] extend::Error),
    #[error(transparent)]
    Data(#[from] data::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
}

impl Cmd {
    #[allow(clippy::too_many_lines)]
    pub async fn run(&self) -> Result<(), Error> {
        let res = self.run_against_rpc_server(None, None).await?.to_envelope();
        let expiration_ledger_seq = match res {
            TxnEnvelopeResult::TxnEnvelope(tx) => {
                println!("{}", tx.to_xdr_base64(Limits::none())?);
                return Ok(());
            }
            TxnEnvelopeResult::Res(res) => res,
        };
        if let Some(ledgers_to_extend) = self.ledgers_to_extend {
            extend::Cmd {
                key: self.key.clone(),
                ledgers_to_extend,
                config: self.config.clone(),
                fee: self.fee.clone(),
                ttl_ledger_only: false,
            }
            .run()
            .await?;
        } else {
            println!("New ttl ledger: {expiration_ledger_seq}");
        }

        Ok(())
    }
}

#[async_trait::async_trait]
impl NetworkRunnable for Cmd {
    type Error = Error;
    type Result = TxnResult<u32>;

    async fn run_against_rpc_server(
        &self,
        args: Option<&global::Args>,
        config: Option<&config::Args>,
    ) -> Result<TxnResult<u32>, Error> {
        let config = config.unwrap_or(&self.config);
        let print = crate::print::Print::new(args.map_or(true, |a| a.quiet));
        let network = config.get_network()?;
        tracing::trace!(?network);
        let entry_keys = self.key.parse_keys(&config.locator, &network)?;
        let client = network.rpc_client()?;
        let source_account = config.source_account().await?;

        // Get the account sequence number
        let account_details = client
            .get_account(&source_account.clone().to_string())
            .await?;
        let sequence: i64 = account_details.seq_num.into();

        let tx = Box::new(Transaction {
            source_account,
            fee: self.fee.fee,
            seq_num: SequenceNumber(sequence + 1),
            cond: Preconditions::None,
            memo: Memo::None,
            operations: vec![Operation {
                source_account: None,
                body: OperationBody::RestoreFootprint(RestoreFootprintOp {
                    ext: ExtensionPoint::V0,
                }),
            }]
            .try_into()?,
            ext: TransactionExt::V1(SorobanTransactionData {
                ext: ExtensionPoint::V0,
                resources: SorobanResources {
                    footprint: LedgerFootprint {
                        read_only: vec![].try_into()?,
                        read_write: entry_keys.try_into()?,
                    },
                    instructions: self.fee.instructions.unwrap_or_default(),
                    read_bytes: 0,
                    write_bytes: 0,
                },
                resource_fee: 0,
            }),
        });
        if self.fee.build_only {
            return Ok(TxnResult::Txn(tx));
        }
        let res = client
            .send_transaction_polling(&config.sign_with_local_key(*tx).await?)
            .await?;
        if args.map_or(true, |a| !a.no_cache) {
            data::write(res.clone().try_into()?, &network.rpc_uri()?)?;
        }
        let meta = res
            .result_meta
            .as_ref()
            .ok_or(Error::MissingOperationResult)?;
        let events = res.events()?;
        tracing::trace!(?meta);
        if !events.is_empty() {
            crate::log::event::all(&events);
            crate::log::event::contract(&events, &print);
        }

        // The transaction from core will succeed regardless of whether it actually found &
        // restored the entry, so we have to inspect the result meta to tell if it worked or not.
        let TransactionMeta::V3(TransactionMetaV3 { operations, .. }) = meta else {
            return Err(Error::LedgerEntryNotFound);
        };
        tracing::debug!("Operations:\nlen:{}\n{operations:#?}", operations.len());

        // Simply check if there is exactly one entry here. We only support extending a single
        // entry via this command (which we should fix separately, but).
        if operations.len() == 0 {
            return Err(Error::LedgerEntryNotFound);
        }

        if operations.len() != 1 {
            tracing::warn!(
                "Unexpected number of operations: {}. Currently only handle one.",
                operations[0].changes.len()
            );
        }
        Ok(TxnResult::Res(
            parse_operations(&operations.to_vec()).ok_or(Error::MissingOperationResult)?,
        ))
    }
}

fn parse_operations(ops: &[OperationMeta]) -> Option<u32> {
    ops.first().and_then(|op| {
        op.changes.iter().find_map(|entry| match entry {
            LedgerEntryChange::Updated(LedgerEntry {
                data:
                    LedgerEntryData::Ttl(TtlEntry {
                        live_until_ledger_seq,
                        ..
                    }),
                ..
            })
            | LedgerEntryChange::Created(LedgerEntry {
                data:
                    LedgerEntryData::Ttl(TtlEntry {
                        live_until_ledger_seq,
                        ..
                    }),
                ..
            }) => Some(*live_until_ledger_seq),
            _ => None,
        })
    })
}
