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
    /// Sequence number to bump to
    #[arg(long)]
    pub bump_to: i64,
}

impl From<&Args> for xdr::OperationBody {
    fn from(args: &Args) -> Self {
        xdr::OperationBody::BumpSequence(xdr::BumpSequenceOp {
            bump_to: args.bump_to.into(),
        })
    }
}

impl From<&Cmd> for xdr::OperationBody {
    fn from(cmd: &Cmd) -> Self {
        xdr::OperationBody::BumpSequence(xdr::BumpSequenceOp {
            bump_to: cmd.op.bump_to.into(),
        })
    }
}
