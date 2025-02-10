use crate::config::address;

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
    pub fn source(&self) -> Option<&address::UnresolvedMuxedAccount> {
        self.operation_source_account.as_ref()
    }
}
