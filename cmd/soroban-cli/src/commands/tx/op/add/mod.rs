use super::super::{global, help, xdr::tx_envelope_from_stdin};
use crate::xdr::{OperationBody, WriteXdr};

pub(crate) use super::super::new;

mod account_merge;
mod args;
mod bump_sequence;
mod change_trust;
mod create_account;
mod manage_data;
mod payment;
mod set_options;
mod set_trustline_flags;

#[derive(Debug, clap::Parser)]
#[allow(clippy::doc_markdown)]
pub enum Cmd {
    #[command(about = help::ACCOUNT_MERGE)]
    AccountMerge(account_merge::Cmd),
    #[command(about = help::BUMP_SEQUENCE)]
    BumpSequence(bump_sequence::Cmd),
    #[command(about = help::CHANGE_TRUST)]
    ChangeTrust(change_trust::Cmd),
    #[command(about = help::CREATE_ACCOUNT)]
    CreateAccount(create_account::Cmd),
    #[command(about = help::MANAGE_DATA)]
    ManageData(manage_data::Cmd),
    #[command(about = help::PAYMENT)]
    Payment(payment::Cmd),
    #[command(about = help::SET_OPTIONS)]
    SetOptions(set_options::Cmd),
    #[command(about = help::SET_TRUSTLINE_FLAGS)]
    SetTrustlineFlags(set_trustline_flags::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    TxXdr(#[from] super::super::xdr::Error),
    #[error(transparent)]
    Xdr(#[from] crate::xdr::Error),
    #[error(transparent)]
    New(#[from] super::super::new::Error),
    #[error(transparent)]
    Tx(#[from] super::super::args::Error),
}

impl TryFrom<&Cmd> for OperationBody {
    type Error = super::super::new::Error;
    fn try_from(cmd: &Cmd) -> Result<Self, Self::Error> {
        Ok(match &cmd {
            Cmd::AccountMerge(account_merge::Cmd { op, .. }) => op.try_into()?,
            Cmd::BumpSequence(bump_sequence::Cmd { op, .. }) => op.into(),
            Cmd::ChangeTrust(change_trust::Cmd { op, .. }) => op.try_into()?,
            Cmd::CreateAccount(create_account::Cmd { op, .. }) => op.try_into()?,
            Cmd::ManageData(manage_data::Cmd { op, .. }) => op.into(),
            Cmd::Payment(payment::Cmd { op, .. }) => op.try_into()?,
            Cmd::SetOptions(set_options::Cmd { op, .. }) => op.try_into()?,
            Cmd::SetTrustlineFlags(set_trustline_flags::Cmd { op, .. }) => op.try_into()?,
        })
    }
}

impl Cmd {
    pub async fn run(&self, _: &global::Args) -> Result<(), Error> {
        let tx_env = tx_envelope_from_stdin()?;
        let op = OperationBody::try_from(self)?;
        let res = match self {
            Cmd::AccountMerge(cmd) => cmd.op.tx.add_op(op, tx_env, cmd.args.source()),
            Cmd::BumpSequence(cmd) => cmd.op.tx.add_op(op, tx_env, cmd.args.source()),
            Cmd::ChangeTrust(cmd) => cmd.op.tx.add_op(op, tx_env, cmd.args.source()),
            Cmd::CreateAccount(cmd) => cmd.op.tx.add_op(op, tx_env, cmd.args.source()),
            Cmd::ManageData(cmd) => cmd.op.tx.add_op(op, tx_env, cmd.args.source()),
            Cmd::Payment(cmd) => cmd.op.tx.add_op(op, tx_env, cmd.args.source()),
            Cmd::SetOptions(cmd) => cmd.op.tx.add_op(op, tx_env, cmd.args.source()),
            Cmd::SetTrustlineFlags(cmd) => cmd.op.tx.add_op(op, tx_env, cmd.args.source()),
        }
        .await?;
        println!("{}", res.to_xdr_base64(crate::xdr::Limits::none())?);
        Ok(())
    }
}
