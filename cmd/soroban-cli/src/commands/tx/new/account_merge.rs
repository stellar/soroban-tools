use clap::{command, Parser};

use crate::{commands::tx, xdr};

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
    /// Muxed Account to merge with, e.g. `GBX...`, 'MBX...'
    #[arg(long)]
    pub account: xdr::MuxedAccount,
}

impl From<&Args> for xdr::OperationBody {
    fn from(cmd: &Args) -> Self {
        xdr::OperationBody::AccountMerge(cmd.account.clone())
    }
}
