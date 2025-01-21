use clap::{command, Parser};

use crate::{commands::tx, tx::builder, xdr};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub tx: tx::Args,
    #[clap(flatten)]
    pub op: Args,
}

#[derive(Debug, clap::Args, Clone)]
pub struct Args {
    /// Account to send to, e.g. `GBX...`
    #[arg(long, visible_alias = "dest")]
    pub destination: xdr::MuxedAccount,
    /// Asset to send, default native, e.i. XLM
    #[arg(long, default_value = "native")]
    pub asset: builder::Asset,
    /// Amount of the aforementioned asset to send. e.g. `10_000_000` (1 XLM)
    #[arg(long)]
    pub amount: builder::Amount,
}

impl From<&Args> for xdr::OperationBody {
    fn from(cmd: &Args) -> Self {
        xdr::OperationBody::Payment(xdr::PaymentOp {
            destination: cmd.destination.clone(),
            asset: cmd.asset.clone().into(),
            amount: cmd.amount.into(),
        })
    }
}
