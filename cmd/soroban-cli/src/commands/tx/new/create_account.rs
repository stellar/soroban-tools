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
    /// Account Id to create, e.g. `GBX...`
    #[arg(long, alias = "dest")]
    pub destination: xdr::AccountId,
    /// Initial balance in stroops of the account, default 1 XLM
    #[arg(long, default_value = "10_000_000")]
    pub starting_balance: builder::Amount,
}

impl From<&Args> for xdr::OperationBody {
    fn from(cmd: &Args) -> Self {
        xdr::OperationBody::CreateAccount(xdr::CreateAccountOp {
            destination: cmd.destination.clone(),
            starting_balance: cmd.starting_balance.into(),
        })
    }
}
