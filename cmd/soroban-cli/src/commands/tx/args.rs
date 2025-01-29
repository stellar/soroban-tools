use crate::{
    commands::{global, txn_result::TxnEnvelopeResult},
    config::{
        self,
        address::{self, UnresolvedMuxedAccount},
        data, network, secret,
    },
    fee,
    rpc::{self, Client, GetTransactionResponse},
    tx::builder::{self, asset, TxExt},
    xdr::{self, Limits, WriteXdr},
};

#[derive(Debug, clap::Args, Clone)]
#[group(skip)]
pub struct Args {
    #[clap(flatten)]
    pub fee: fee::Args,
    #[clap(flatten)]
    pub config: config::Args,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Rpc(#[from] rpc::Error),
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error(transparent)]
    Network(#[from] network::Error),
    #[error(transparent)]
    Secret(#[from] secret::Error),
    #[error(transparent)]
    Tx(#[from] builder::Error),
    #[error(transparent)]
    Data(#[from] data::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error(transparent)]
    Address(#[from] address::Error),
    #[error(transparent)]
    Asset(#[from] asset::Error),
    #[error(transparent)]
    TxXdr(#[from] super::xdr::Error),
}

impl Args {
    pub async fn tx(&self, body: impl Into<xdr::OperationBody>) -> Result<xdr::Transaction, Error> {
        let source_account = self.source_account()?;
        let seq_num = self
            .config
            .next_sequence_number(source_account.clone().account_id())
            .await?;
        // Once we have a way to add operations this will be updated to allow for a different source account
        let operation = xdr::Operation {
            source_account: None,
            body: body.into(),
        };
        Ok(xdr::Transaction::new_tx(
            source_account,
            self.fee.fee,
            seq_num,
            operation,
        ))
    }

    pub fn client(&self) -> Result<Client, Error> {
        let network = self.config.get_network()?;
        Ok(Client::new(&network.rpc_url)?)
    }

    pub async fn handle(
        &self,
        op: impl Into<xdr::OperationBody>,
        global_args: &global::Args,
    ) -> Result<TxnEnvelopeResult<GetTransactionResponse>, Error> {
        let tx = self.tx(op).await?;
        self.handle_tx(tx, global_args).await
    }
    pub async fn handle_and_print(
        &self,
        op: impl Into<xdr::OperationBody>,
        global_args: &global::Args,
    ) -> Result<(), Error> {
        let res = self.handle(op, global_args).await?;
        if let TxnEnvelopeResult::TxnEnvelope(tx) = res {
            println!("{}", tx.to_xdr_base64(Limits::none())?);
        };
        Ok(())
    }

    pub async fn handle_tx(
        &self,
        tx: xdr::Transaction,
        args: &global::Args,
    ) -> Result<TxnEnvelopeResult<GetTransactionResponse>, Error> {
        let network = self.config.get_network()?;
        let client = Client::new(&network.rpc_url)?;
        if self.fee.build_only {
            return Ok(TxnEnvelopeResult::TxnEnvelope(Box::new(tx.into())));
        }

        let txn_resp = client
            .send_transaction_polling(&self.config.sign_with_local_key(tx).await?)
            .await?;

        if !args.no_cache {
            data::write(txn_resp.clone().try_into().unwrap(), &network.rpc_uri()?)?;
        }

        Ok(TxnEnvelopeResult::Res(txn_resp))
    }

    pub fn source_account(&self) -> Result<xdr::MuxedAccount, Error> {
        Ok(self.config.source_account()?)
    }

    pub fn resolve_muxed_address(
        &self,
        address: &UnresolvedMuxedAccount,
    ) -> Result<xdr::MuxedAccount, Error> {
        Ok(address.resolve_muxed_account(&self.config.locator, self.config.hd_path)?)
    }

    pub fn resolve_account_id(
        &self,
        address: &UnresolvedMuxedAccount,
    ) -> Result<xdr::AccountId, Error> {
        Ok(address
            .resolve_muxed_account(&self.config.locator, self.config.hd_path)?
            .account_id())
    }

    pub fn add_op(
        &self,
        op_body: impl Into<xdr::OperationBody>,
        tx_env: xdr::TransactionEnvelope,
        op_source: Option<&address::UnresolvedMuxedAccount>,
    ) -> Result<xdr::TransactionEnvelope, Error> {
        let source_account = op_source
            .map(|a| self.resolve_muxed_address(a))
            .transpose()?;
        let op = xdr::Operation {
            source_account,
            body: op_body.into(),
        };
        Ok(super::xdr::add_op(tx_env, op)?)
    }

    pub fn resolve_asset(&self, asset: &builder::Asset) -> Result<xdr::Asset, Error> {
        Ok(asset.resolve(&self.config.locator)?)
    }
}
