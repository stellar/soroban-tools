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
#[allow(clippy::struct_excessive_bools, clippy::doc_markdown)]
pub struct Args {
    /// Account to set trustline flags for, e.g. `GBX...`, or alias, or muxed account, `M123...``
    #[arg(long)]
    pub trustor: address::UnresolvedMuxedAccount,
    /// Asset to set trustline flags for
    #[arg(long)]
    pub asset: builder::Asset,
    #[arg(long, conflicts_with = "clear_authorize")]
    /// Signifies complete authorization allowing an account to transact freely with the asset to make and receive payments and place orders.
    pub set_authorize: bool,
    #[arg(long, conflicts_with = "clear_authorize_to_maintain_liabilities")]
    /// Denotes limited authorization that allows an account to maintain current orders but not to otherwise transact with the asset.
    pub set_authorize_to_maintain_liabilities: bool,
    #[arg(long, conflicts_with = "clear_trustline_clawback_enabled")]
    /// Enables the issuing account to take back (burning) all of the asset. See our section on Clawbacks:
    /// https://developers.stellar.org/docs/learn/encyclopedia/transactions-specialized/clawbacks
    pub set_trustline_clawback_enabled: bool,
    #[arg(long)]
    pub clear_authorize: bool,
    #[arg(long)]
    pub clear_authorize_to_maintain_liabilities: bool,
    #[arg(long)]
    pub clear_trustline_clawback_enabled: bool,
}

impl TryFrom<&Cmd> for xdr::OperationBody {
    type Error = tx::args::Error;
    fn try_from(cmd: &Cmd) -> Result<Self, Self::Error> {
        let mut set_flags = 0;
        let mut set_flag = |flag: xdr::TrustLineFlags| set_flags |= flag as u32;

        if cmd.op.set_authorize {
            set_flag(xdr::TrustLineFlags::AuthorizedFlag);
        };
        if cmd.op.set_authorize_to_maintain_liabilities {
            set_flag(xdr::TrustLineFlags::AuthorizedToMaintainLiabilitiesFlag);
        };
        if cmd.op.set_trustline_clawback_enabled {
            set_flag(xdr::TrustLineFlags::TrustlineClawbackEnabledFlag);
        };

        let mut clear_flags = 0;
        let mut clear_flag = |flag: xdr::TrustLineFlags| clear_flags |= flag as u32;
        if cmd.op.clear_authorize {
            clear_flag(xdr::TrustLineFlags::AuthorizedFlag);
        };
        if cmd.op.clear_authorize_to_maintain_liabilities {
            clear_flag(xdr::TrustLineFlags::AuthorizedToMaintainLiabilitiesFlag);
        };
        if cmd.op.clear_trustline_clawback_enabled {
            clear_flag(xdr::TrustLineFlags::TrustlineClawbackEnabledFlag);
        };

        Ok(xdr::OperationBody::SetTrustLineFlags(
            xdr::SetTrustLineFlagsOp {
                trustor: cmd.tx.resolve_account_id(&cmd.op.trustor)?,
                asset: cmd.tx.resolve_asset(&cmd.op.asset)?,
                clear_flags,
                set_flags,
            },
        ))
    }
}
