use std::array::TryFromSliceError;
use std::{fmt::Debug, fs, io};

use clap::Parser;
use ed25519_dalek;
use ed25519_dalek::Signer;
use hex::FromHexError;
use rand::Rng;
use sha2::{Digest, Sha256};
use soroban_env_host::xdr::{
    DecoratedSignature, Error as XdrError, Hash, HashIdPreimageEd25519ContractId, HostFunction,
    InvokeHostFunctionOp, LedgerFootprint, LedgerKey::ContractData, LedgerKeyContractData, Memo,
    MuxedAccount, Operation, OperationBody, Preconditions, ScObject,
    ScStatic::LedgerKeyContractCode, ScVal, SequenceNumber, Signature, SignatureHint, Transaction,
    TransactionEnvelope, TransactionExt, TransactionSignaturePayload,
    TransactionSignaturePayloadTaggedTransaction, TransactionV1Envelope, Uint256, VecM, WriteXdr,
};
use soroban_env_host::HostError;
use stellar_strkey::StrkeyPrivateKeyEd25519;

use crate::snapshot::{self, get_default_ledger_info};
use crate::utils;

#[derive(Parser, Debug)]
pub struct Cmd {
    #[clap(long = "id")]
    /// Contract ID to deploy to
    contract_id: String,
    /// WASM file to deploy
    #[clap(long, parse(from_os_str))]
    wasm: std::path::PathBuf,
    /// File to persist ledger state
    #[clap(long, parse(from_os_str), default_value(".soroban/ledger.json"))]
    ledger_file: std::path::PathBuf,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Host(#[from] HostError),
    #[error("internal conversion error: {0}")]
    TryFromSliceError(#[from] TryFromSliceError),
    #[error("xdr processing error: {0}")]
    Xdr(#[from] XdrError),
    #[error("reading file {filepath}: {error}")]
    CannotReadLedgerFile {
        filepath: std::path::PathBuf,
        error: snapshot::Error,
    },
    #[error("reading file {filepath}: {error}")]
    CannotReadContractFile {
        filepath: std::path::PathBuf,
        error: io::Error,
    },
    #[error("committing file {filepath}: {error}")]
    CannotCommitLedgerFile {
        filepath: std::path::PathBuf,
        error: snapshot::Error,
    },
    #[error("cannot parse contract ID {contract_id}: {error}")]
    CannotParseContractId {
        contract_id: String,
        error: FromHexError,
    },
    #[error("cannot parse private key")]
    CannotParsePrivateKey,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let contract_id: [u8; 32] =
            utils::contract_id_from_str(&self.contract_id).map_err(|e| {
                Error::CannotParseContractId {
                    contract_id: self.contract_id.clone(),
                    error: e,
                }
            })?;
        let contract = fs::read(&self.wasm).map_err(|e| Error::CannotReadContractFile {
            filepath: self.wasm.clone(),
            error: e,
        })?;

        let mut state =
            snapshot::read(&self.ledger_file).map_err(|e| Error::CannotReadLedgerFile {
                filepath: self.ledger_file.clone(),
                error: e,
            })?;
        utils::add_contract_to_ledger_entries(&mut state.1, contract_id, contract)?;

        snapshot::commit(state.1, get_default_ledger_info(), [], &self.ledger_file).map_err(
            |e| Error::CannotCommitLedgerFile {
                filepath: self.ledger_file.clone(),
                error: e,
            },
        )?;
        Ok(())
    }
}

fn build_create_contract_tx(
    contract: Vec<u8>,
    sequence: i64,
    fee: u32,
    network_passphrase: &str,
    key: ed25519_dalek::Keypair,
) -> Result<TransactionEnvelope, Error> {
    let salt = rand::thread_rng().gen::<[u8; 32]>();

    let separator =
        b"create_contract_from_ed25519(contract: Vec<u8>, salt: u256, key: u256, sig: Vec<u8>)";
    let contract_hash = Sha256::new()
        .chain_update(separator)
        .chain_update(salt)
        .chain_update(contract.clone())
        .finalize();

    let contract_signature = key.sign(&contract_hash);

    let preimage = HashIdPreimageEd25519ContractId {
        ed25519: Uint256(key.secret.as_bytes().clone()),
        salt: Uint256(salt.into()),
    };
    let preimage_xdr = preimage.to_xdr()?;
    let contract_id = Sha256::digest(preimage_xdr);

    // TODO: clean up duplicated code and check whether the type conversions here make sense
    let contract_parameter = ScVal::Object(Some(ScObject::Bytes(contract.try_into()?)));
    let salt_parameter = ScVal::Object(Some(ScObject::Bytes(salt.try_into()?)));
    let public_key_parameter =
        ScVal::Object(Some(ScObject::Bytes(key.public.as_bytes().try_into()?)));
    let signature_parameter = ScVal::Object(Some(ScObject::Bytes(
        contract_signature.to_bytes().try_into()?,
    )));

    // TODO: reorder code properly
    let lk = ContractData(LedgerKeyContractData {
        contract_id: Hash(contract_id.into()),
        key: ScVal::Static(LedgerKeyContractCode),
    });

    let parameters: VecM<ScVal, 256000> = vec![
        contract_parameter,
        salt_parameter,
        public_key_parameter,
        signature_parameter,
    ]
    .try_into()?;

    let op = Operation {
        source_account: None,
        body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
            function: HostFunction::CreateContract,
            parameters: parameters.into(),
            footprint: LedgerFootprint {
                read_only: Default::default(),
                read_write: vec![lk].try_into()?,
            },
        }),
    };
    let tx = Transaction {
        source_account: MuxedAccount::Ed25519(Uint256(key.public.as_bytes().clone())),
        fee: fee,
        seq_num: SequenceNumber(sequence),
        cond: Preconditions::None,
        memo: Memo::None,
        operations: vec![op].try_into()?,
        ext: TransactionExt::V0,
    };

    // sign the transaction
    let passphrase_hash = Sha256::digest(network_passphrase);
    let signature_payload = TransactionSignaturePayload {
        network_id: Hash(passphrase_hash.into()),
        tagged_transaction: TransactionSignaturePayloadTaggedTransaction::Tx(tx.clone()),
    };
    let tx_hash = Sha256::digest(signature_payload.to_xdr()?);
    let tx_signature = key.sign(&tx_hash);

    let decorated_signature = DecoratedSignature {
        hint: SignatureHint(key.public.to_bytes()[28..].try_into()?),
        signature: Signature(tx_signature.to_bytes().try_into()?),
    };

    let envelope = TransactionEnvelope::Tx(TransactionV1Envelope {
        tx: tx,
        signatures: vec![decorated_signature].try_into()?,
    });

    Ok(envelope)
}

fn parse_private_key(strkey: &str) -> Result<ed25519_dalek::Keypair, Error> {
    let seed =
        StrkeyPrivateKeyEd25519::from_string(&strkey).map_err(|_| Error::CannotParsePrivateKey)?;
    let secret_key =
        ed25519_dalek::SecretKey::from_bytes(&seed.0).map_err(|_| Error::CannotParsePrivateKey)?;
    let public_key = (&secret_key).into();
    Ok(ed25519_dalek::Keypair {
        secret: secret_key,
        public: public_key,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_private_key() {
        let seed = "SBFGFF27Y64ZUGFAIG5AMJGQODZZKV2YQKAVUUN4HNE24XZXD2OEUVUP";
        let keypair = parse_private_key(seed).unwrap();

        let expected_public_key: [u8; 32] = [
            0x31, 0x40, 0xf1, 0x40, 0x99, 0xa7, 0x4c, 0x90, 0xd4, 0x62, 0x48, 0xec, 0x8d, 0xef,
            0xb3, 0x38, 0xc8, 0x2c, 0xe2, 0x42, 0x85, 0xc9, 0xf7, 0xb8, 0x95, 0xce, 0xdd, 0x6f,
            0x96, 0x47, 0x82, 0x96,
        ];
        assert_eq!(expected_public_key, keypair.public.to_bytes());

        let expected_private_key: [u8; 32] = [
            0x4a, 0x62, 0x97, 0x5f, 0xc7, 0xb9, 0x9a, 0x18, 0xa0, 0x41, 0xba, 0x6, 0x24, 0xd0,
            0x70, 0xf3, 0x95, 0x57, 0x58, 0x82, 0x81, 0x5a, 0x51, 0xbc, 0x3b, 0x49, 0xae, 0x5f,
            0x37, 0x1e, 0x9c, 0x4a,
        ];
        assert_eq!(expected_private_key, keypair.secret.to_bytes());
    }

    #[test]
    fn test_build_create_contract() {
        let result = build_create_contract_tx(
            b"foo".to_vec(),
            300,
            1,
            "Public Global Stellar Network ; September 2015",
            parse_private_key("SBFGFF27Y64ZUGFAIG5AMJGQODZZKV2YQKAVUUN4HNE24XZXD2OEUVUP").unwrap(),
        );

        assert!(result.is_ok());
    }
}
