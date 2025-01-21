use std::{
    io::{stdin, Read},
    path::PathBuf,
};

use crate::xdr::{
    Limits, Operation, ReadXdr, Transaction, TransactionEnvelope, TransactionV1Envelope,
};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to decode XDR from base64")]
    Base64Decode,
    #[error("failed to decode XDR from file: {0}")]
    FileDecode(PathBuf),
    #[error("failed to decode XDR from stdin")]
    StdinDecode,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("only transaction v1 is supported")]
    OnlyTransactionV1Supported,
    #[error("too many operations, limited to 100 operations in a transaction")]
    TooManyOperations,
}

pub fn tx_envelope_from_stdin() -> Result<TransactionEnvelope, Error> {
    from_stdin()
}
pub fn from_stdin<T: ReadXdr>() -> Result<T, Error> {
    let mut buf = String::new();
    let _ = stdin()
        .read_to_string(&mut buf)
        .map_err(|_| Error::StdinDecode)?;
    T::from_xdr_base64(buf.trim(), Limits::none()).map_err(|_| Error::StdinDecode)
}

pub fn unwrap_envelope_v1(tx_env: TransactionEnvelope) -> Result<Transaction, Error> {
    let TransactionEnvelope::Tx(TransactionV1Envelope { tx, .. }) = tx_env else {
        return Err(Error::OnlyTransactionV1Supported);
    };
    Ok(tx)
}

pub fn add_op(tx_env: TransactionEnvelope, op: Operation) -> Result<TransactionEnvelope, Error> {
    let mut tx = unwrap_envelope_v1(tx_env)?;
    let mut ops = tx.operations.to_vec();
    ops.push(op);
    tx.operations = ops.try_into().map_err(|_| Error::TooManyOperations)?;
    Ok(tx.into())
}
