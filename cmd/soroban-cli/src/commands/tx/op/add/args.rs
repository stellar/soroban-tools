use crate::{commands::tx, config::address, xdr};

#[derive(Debug, clap::Args, Clone)]
#[group(skip)]
pub struct Args {
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
        tx: &tx::args::Args,
    ) -> Result<xdr::TransactionEnvelope, tx::args::Error> {
        tx.add_op(op_body, tx_env, self.operation_source_account.as_ref())
    }
}
