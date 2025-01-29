use crate::xdr;

pub mod auth;
pub mod cost;
pub mod event;
pub mod footprint;

pub use auth::*;
pub use cost::*;
pub use footprint::*;

pub fn extract_events(tx_meta: &xdr::TransactionMeta) -> Vec<xdr::DiagnosticEvent> {
    match tx_meta {
        xdr::TransactionMeta::V3(xdr::TransactionMetaV3 {
            soroban_meta: Some(meta),
            ..
        }) => meta.diagnostic_events.to_vec(),
        _ => Vec::new(),
    }
}
