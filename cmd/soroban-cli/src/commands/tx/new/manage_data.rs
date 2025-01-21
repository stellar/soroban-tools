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
    /// String up to 64 bytes long.
    /// If this is a new Name it will add the given name/value pair to the account.
    /// If this Name is already present then the associated value will be modified.
    #[arg(long)]
    pub data_name: xdr::StringM<64>,
    /// Up to 64 bytes long hex string
    /// If not present then the existing Name will be deleted.
    /// If present then this value will be set in the `DataEntry`.
    #[arg(long)]
    pub data_value: Option<xdr::BytesM<64>>,
}

impl From<&Args> for xdr::OperationBody {
    fn from(cmd: &Args) -> Self {
        let data_value = cmd.data_value.clone().map(Into::into);
        let data_name = cmd.data_name.clone().into();
        xdr::OperationBody::ManageData(xdr::ManageDataOp {
            data_name,
            data_value,
        })
    }
}
