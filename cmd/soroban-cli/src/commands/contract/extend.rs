use std::{fmt::Debug, path::Path, str::FromStr};

use crate::{
    print::Print,
    xdr::{
        Error as XdrError, ExtendFootprintTtlOp, ExtensionPoint, LedgerEntry, LedgerEntryChange,
        LedgerEntryData, LedgerFootprint, Limits, Memo, Operation, OperationBody, Preconditions,
        SequenceNumber, SorobanResources, SorobanTransactionData, Transaction, TransactionExt,
        TransactionMeta, TransactionMetaV3, TtlEntry, WriteXdr,
    },
};
use clap::{command, Parser};

use crate::{
    assembled::simulate_and_assemble_transaction,
    commands::{
        global,
        txn_result::{TxnEnvelopeResult, TxnResult},
        NetworkRunnable,
    },
    config::{self, data, locator, network},
    key, rpc, wasm, Pwd,
};

const MAX_LEDGERS_TO_EXTEND: u32 = 535_679;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Number of ledgers to extend the entries
    #[arg(long, required = true)]
    pub ledgers_to_extend: u32,
    /// Only print the new Time To Live ledger
    #[arg(long)]
    pub ttl_ledger_only: bool,
    #[command(flatten)]
    pub key: key::Args,
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

    #[error(transparent)]
    Config(#[from] config::Error),
    #[error("either `--key` or `--key-xdr` are required")]
    KeyIsRequired,
    #[error("xdr processing error: {0}")]
    Xdr(#[from] XdrError),
    #[error("Ledger entry not found")]
    LedgerEntryNotFound,
    #[error("missing operation result")]
    MissingOperationResult,
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Wasm(#[from] wasm::Error),
    #[error(transparent)]
    Key(#[from] key::Error),
    #[error(transparent)]
    Data(#[from] data::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
}

impl Cmd {
    #[allow(clippy::too_many_lines)]
    pub async fn run(&self) -> Result<(), Error> {
        let res = self.run_against_rpc_server(None, None).await?.to_envelope();
        match res {
            TxnEnvelopeResult::TxnEnvelope(tx) => println!("{}", tx.to_xdr_base64(Limits::none())?),
            TxnEnvelopeResult::Res(ttl_ledger) => {
                if self.ttl_ledger_only {
                    println!("{ttl_ledger}");
                } else {
                    println!("New ttl ledger: {ttl_ledger}");
                }
            }
        }

        Ok(())
    }

    fn ledgers_to_extend(&self) -> u32 {
        let res = u32::min(self.ledgers_to_extend, MAX_LEDGERS_TO_EXTEND);
        if res < self.ledgers_to_extend {
            tracing::warn!(
                "Ledgers to extend is too large, using max value of {MAX_LEDGERS_TO_EXTEND}"
            );
        }
        res
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
    ) -> Result<TxnResult<u32>, Self::Error> {
        let config = config.unwrap_or(&self.config);
        let print = Print::new(args.map_or(false, |a| a.quiet));
        let network = config.get_network()?;
        tracing::trace!(?network);
        let keys = self.key.parse_keys(&config.locator, &network)?;
        let client = network.rpc_client()?;
        let source_account = config.source_account()?;
        let extend_to = self.ledgers_to_extend();

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
                body: OperationBody::ExtendFootprintTtl(ExtendFootprintTtlOp {
                    ext: ExtensionPoint::V0,
                    extend_to,
                }),
            }]
            .try_into()?,
            ext: TransactionExt::V1(SorobanTransactionData {
                ext: ExtensionPoint::V0,
                resources: SorobanResources {
                    footprint: LedgerFootprint {
                        read_only: keys.clone().try_into()?,
                        read_write: vec![].try_into()?,
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
        let tx = simulate_and_assemble_transaction(&client, &tx)
            .await?
            .transaction()
            .clone();
        let res = client
            .send_transaction_polling(&config.sign_with_local_key(tx).await?)
            .await?;
        if args.map_or(true, |a| !a.no_cache) {
            data::write(res.clone().try_into()?, &network.rpc_uri()?)?;
        }

        let events = res.events()?;
        if !events.is_empty() {
            crate::log::event::all(&events);
            crate::log::event::contract(&events, &print);
        }
        let meta = res.result_meta.ok_or(Error::MissingOperationResult)?;

        // The transaction from core will succeed regardless of whether it actually found & extended
        // the entry, so we have to inspect the result meta to tell if it worked or not.
        let TransactionMeta::V3(TransactionMetaV3 { operations, .. }) = meta else {
            return Err(Error::LedgerEntryNotFound);
        };

        // Simply check if there is exactly one entry here. We only support extending a single
        // entry via this command (which we should fix separately, but).
        if operations.len() == 0 {
            return Err(Error::LedgerEntryNotFound);
        }

        if operations[0].changes.is_empty() {
            let entry = client.get_full_ledger_entries(&keys).await?;
            let extension = entry.entries[0].live_until_ledger_seq;
            if entry.latest_ledger + i64::from(extend_to) < i64::from(extension) {
                return Ok(TxnResult::Res(extension));
            }
        }

        match (&operations[0].changes[0], &operations[0].changes[1]) {
            (
                LedgerEntryChange::State(_),
                LedgerEntryChange::Updated(LedgerEntry {
                    data:
                        LedgerEntryData::Ttl(TtlEntry {
                            live_until_ledger_seq,
                            ..
                        }),
                    ..
                }),
            ) => Ok(TxnResult::Res(*live_until_ledger_seq)),
            _ => Err(Error::LedgerEntryNotFound),
        }
    }
}
