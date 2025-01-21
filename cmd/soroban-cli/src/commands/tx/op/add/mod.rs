use clap::Parser;

use super::super::{global, help, xdr::tx_envelope_from_stdin};
use crate::xdr::WriteXdr;

pub(crate) use super::super::{new, xdr};

mod account_merge;
mod args;
mod bump_sequence;
mod change_trust;
mod create_account;
mod manage_data;
mod payment;
mod set_options;
mod set_trustline_flags;

#[derive(Debug, Parser)]
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
    Args(#[from] args::Error),
    #[error(transparent)]
    TxXdr(#[from] super::super::xdr::Error),
    #[error(transparent)]
    Xdr(#[from] crate::xdr::Error),
}

impl Cmd {
    pub fn run(&self, _: &global::Args) -> Result<(), Error> {
        let tx_env = tx_envelope_from_stdin()?;
        let res = match self {
            Cmd::AccountMerge(cmd) => cmd.args.add_op(&cmd.op, tx_env),
            Cmd::BumpSequence(cmd) => cmd.args.add_op(&cmd.op, tx_env),
            Cmd::ChangeTrust(cmd) => cmd.args.add_op(&cmd.op, tx_env),
            Cmd::CreateAccount(cmd) => cmd.args.add_op(&cmd.op, tx_env),
            Cmd::ManageData(cmd) => cmd.args.add_op(&cmd.op, tx_env),
            Cmd::Payment(cmd) => cmd.args.add_op(&cmd.op, tx_env),
            Cmd::SetOptions(cmd) => cmd.args.add_op(&cmd.op, tx_env),
            Cmd::SetTrustlineFlags(cmd) => cmd.args.add_op(&cmd.op, tx_env),
        }?;
        println!("{}", res.to_xdr_base64(crate::xdr::Limits::none())?);
        Ok(())
    }
}
