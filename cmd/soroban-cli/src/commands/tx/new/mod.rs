use clap::Parser;
use soroban_sdk::xdr::OperationBody;

use super::global;

pub mod account_merge;
pub mod bump_sequence;
pub mod change_trust;
pub mod create_account;
pub mod manage_data;
pub mod payment;
pub mod set_options;
pub mod set_trustline_flags;

#[derive(Debug, Parser)]
#[allow(clippy::doc_markdown)]
pub enum Cmd {
    #[command(about = super::help::ACCOUNT_MERGE)]
    AccountMerge(account_merge::Cmd),
    #[command(about = super::help::BUMP_SEQUENCE)]
    BumpSequence(bump_sequence::Cmd),
    #[command(about = super::help::CHANGE_TRUST)]
    ChangeTrust(change_trust::Cmd),
    #[command(about = super::help::CREATE_ACCOUNT)]
    CreateAccount(create_account::Cmd),
    #[command(about = super::help::MANAGE_DATA)]
    ManageData(manage_data::Cmd),
    #[command(about = super::help::PAYMENT)]
    Payment(payment::Cmd),
    #[command(about = super::help::SET_OPTIONS)]
    SetOptions(set_options::Cmd),
    #[command(about = super::help::SET_TRUSTLINE_FLAGS)]
    SetTrustlineFlags(set_trustline_flags::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Tx(#[from] super::args::Error),
}

impl TryFrom<&Cmd> for OperationBody {
    type Error = super::args::Error;
    fn try_from(cmd: &Cmd) -> Result<Self, Self::Error> {
        Ok(match cmd {
            Cmd::AccountMerge(cmd) => cmd.try_into()?,
            Cmd::BumpSequence(cmd) => cmd.into(),
            Cmd::ChangeTrust(cmd) => cmd.try_into()?,
            Cmd::CreateAccount(cmd) => cmd.try_into()?,
            Cmd::ManageData(cmd) => cmd.into(),
            Cmd::Payment(cmd) => cmd.try_into()?,
            Cmd::SetOptions(cmd) => cmd.try_into()?,
            Cmd::SetTrustlineFlags(cmd) => cmd.try_into()?,
        })
    }
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let op = OperationBody::try_from(self)?;
        match self {
            Cmd::AccountMerge(cmd) => cmd.tx.handle_and_print(op, global_args).await,
            Cmd::BumpSequence(cmd) => cmd.tx.handle_and_print(op, global_args).await,
            Cmd::ChangeTrust(cmd) => cmd.tx.handle_and_print(op, global_args).await,
            Cmd::CreateAccount(cmd) => cmd.tx.handle_and_print(op, global_args).await,
            Cmd::ManageData(cmd) => cmd.tx.handle_and_print(op, global_args).await,
            Cmd::Payment(cmd) => cmd.tx.handle_and_print(op, global_args).await,
            Cmd::SetOptions(cmd) => cmd.tx.handle_and_print(op, global_args).await,
            Cmd::SetTrustlineFlags(cmd) => cmd.tx.handle_and_print(op, global_args).await,
        }?;
        Ok(())
    }
}
