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
    #[arg(long)]
    pub line: builder::Asset,
    /// Limit for the trust line, 0 to remove the trust line
    #[arg(long, default_value = i64::MAX.to_string())]
    pub limit: i64,
}

impl TryFrom<&Cmd> for xdr::OperationBody {
    type Error = tx::args::Error;
    fn try_from(cmd: &Cmd) -> Result<Self, Self::Error> {
        let asset = cmd.tx.resolve_asset(&cmd.op.line)?;
        let line = match asset {
            xdr::Asset::CreditAlphanum4(asset) => xdr::ChangeTrustAsset::CreditAlphanum4(asset),
            xdr::Asset::CreditAlphanum12(asset) => xdr::ChangeTrustAsset::CreditAlphanum12(asset),
            xdr::Asset::Native => xdr::ChangeTrustAsset::Native,
        };
        Ok(xdr::OperationBody::ChangeTrust(xdr::ChangeTrustOp {
            line,
            limit: cmd.op.limit,
        }))
    }
}
