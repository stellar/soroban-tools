use super::xdr::add_op;
use crate::{
    config::{address, locator},
    xdr,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Address(#[from] address::Error),
    #[error(transparent)]
    TxXdr(#[from] super::xdr::Error),
}

#[derive(Debug, clap::Args, Clone)]
#[group(skip)]
pub struct Args {
    #[clap(flatten)]
    pub locator: locator::Args,
    /// Source account used for the operation
    #[arg(
        long,
        visible_alias = "op-source",
        env = "STELLAR_OPERATION_SOURCE_ACCOUNT"
    )]
    pub operation_source_account: Option<address::UnresolvedMuxedAccount>,
}

impl Args {
    pub fn add_op(
        &self,
        op_body: impl Into<xdr::OperationBody>,
        tx_env: xdr::TransactionEnvelope,
    ) -> Result<xdr::TransactionEnvelope, Error> {
        let source_account = self
            .operation_source_account
            .as_ref()
            .map(|a| a.resolve_muxed_account(&self.locator, None))
            .transpose()?;
        let op = xdr::Operation {
            source_account,
            body: op_body.into(),
        };
        Ok(add_op(tx_env, op)?)
    }
}
