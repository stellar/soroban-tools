use clap::{command, Parser};

use crate::{commands::tx, config::address, tx::builder, xdr};

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
    #[arg(long)]
    pub destination: address::UnresolvedMuxedAccount,
    /// Initial balance in stroops of the account, default 1 XLM
    #[arg(long, default_value = "10_000_000")]
    pub starting_balance: builder::Amount,
}

impl TryFrom<&Cmd> for xdr::OperationBody {
    type Error = tx::args::Error;
    fn try_from(cmd: &Cmd) -> Result<Self, Self::Error> {
        Ok(xdr::OperationBody::CreateAccount(xdr::CreateAccountOp {
            destination: cmd.tx.resolve_account_id(&cmd.op.destination)?,
            starting_balance: cmd.op.starting_balance.into(),
        }))
    }
}
