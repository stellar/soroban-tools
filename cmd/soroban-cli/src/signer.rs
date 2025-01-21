use ed25519_dalek::ed25519::signature::Signer as _;
use keyring::StellarEntry;
use sha2::{Digest, Sha256};

use crate::xdr::{
    self, AccountId, DecoratedSignature, Hash, HashIdPreimage, HashIdPreimageSorobanAuthorization,
    InvokeHostFunctionOp, Limits, Operation, OperationBody, PublicKey, ScAddress, ScMap, ScSymbol,
    ScVal, Signature, SignatureHint, SorobanAddressCredentials, SorobanAuthorizationEntry,
    SorobanAuthorizedFunction, SorobanCredentials, Transaction, TransactionEnvelope,
    TransactionV1Envelope, Uint256, VecM, WriteXdr,
};

use crate::{config::network::Network, print::Print, utils::transaction_hash};

pub mod keyring;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Contract addresses are not supported to sign auth entries {address}")]
    ContractAddressAreNotSupported { address: String },
    #[error(transparent)]
    Ed25519(#[from] ed25519_dalek::SignatureError),
    #[error("Missing signing key for account {address}")]
    MissingSignerForAddress { address: String },
    #[error(transparent)]
    TryFromSlice(#[from] std::array::TryFromSliceError),
    #[error("User cancelled signing, perhaps need to add -y")]
    UserCancelledSigning,
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error("Only Transaction envelope V1 type is supported")]
    UnsupportedTransactionEnvelopeType,
    #[error(transparent)]
    Url(#[from] url::ParseError),
    #[error(transparent)]
    Open(#[from] std::io::Error),
    #[error("Returning a signature from Lab is not yet supported; Transaction can be found and submitted in lab")]
    ReturningSignatureFromLab,
    #[error(transparent)]
    Keyring(#[from] keyring::Error),
}

fn requires_auth(txn: &Transaction) -> Option<xdr::Operation> {
    let [op @ Operation {
        body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp { auth, .. }),
        ..
    }] = txn.operations.as_slice()
    else {
        return None;
    };
    matches!(
        auth.first().map(|x| &x.root_invocation.function),
        Some(&SorobanAuthorizedFunction::ContractFn(_))
    )
    .then(move || op.clone())
}

// Use the given source_key and signers, to sign all SorobanAuthorizationEntry's in the given
// transaction. If unable to sign, return an error.
pub fn sign_soroban_authorizations(
    raw: &Transaction,
    source_key: &ed25519_dalek::SigningKey,
    signers: &[ed25519_dalek::SigningKey],
    signature_expiration_ledger: u32,
    network_passphrase: &str,
) -> Result<Option<Transaction>, Error> {
    let mut tx = raw.clone();
    let Some(mut op) = requires_auth(&tx) else {
        return Ok(None);
    };

    let Operation {
        body: OperationBody::InvokeHostFunction(ref mut body),
        ..
    } = op
    else {
        return Ok(None);
    };

    let network_id = Hash(Sha256::digest(network_passphrase.as_bytes()).into());

    let verification_key = source_key.verifying_key();
    let source_address = verification_key.as_bytes();

    let signed_auths = body
        .auth
        .as_slice()
        .iter()
        .map(|raw_auth| {
            let mut auth = raw_auth.clone();
            let SorobanAuthorizationEntry {
                credentials: SorobanCredentials::Address(ref mut credentials),
                ..
            } = auth
            else {
                // Doesn't need special signing
                return Ok(auth);
            };
            let SorobanAddressCredentials { ref address, .. } = credentials;

            // See if we have a signer for this authorizationEntry
            // If not, then we Error
            let needle = match address {
                ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(ref a)))) => a,
                ScAddress::Contract(Hash(c)) => {
                    // This address is for a contract. This means we're using a custom
                    // smart-contract account. Currently the CLI doesn't support that yet.
                    return Err(Error::MissingSignerForAddress {
                        address: stellar_strkey::Strkey::Contract(stellar_strkey::Contract(*c))
                            .to_string(),
                    });
                }
            };
            let signer = if let Some(s) = signers
                .iter()
                .find(|s| needle == s.verifying_key().as_bytes())
            {
                s
            } else if needle == source_address {
                // This is the source address, so we can sign it
                source_key
            } else {
                // We don't have a signer for this address
                return Err(Error::MissingSignerForAddress {
                    address: stellar_strkey::Strkey::PublicKeyEd25519(
                        stellar_strkey::ed25519::PublicKey(*needle),
                    )
                    .to_string(),
                });
            };

            sign_soroban_authorization_entry(
                raw_auth,
                signer,
                signature_expiration_ledger,
                &network_id,
            )
        })
        .collect::<Result<Vec<_>, Error>>()?;

    body.auth = signed_auths.try_into()?;
    tx.operations = vec![op].try_into()?;
    Ok(Some(tx))
}

fn sign_soroban_authorization_entry(
    raw: &SorobanAuthorizationEntry,
    signer: &ed25519_dalek::SigningKey,
    signature_expiration_ledger: u32,
    network_id: &Hash,
) -> Result<SorobanAuthorizationEntry, Error> {
    let mut auth = raw.clone();
    let SorobanAuthorizationEntry {
        credentials: SorobanCredentials::Address(ref mut credentials),
        ..
    } = auth
    else {
        // Doesn't need special signing
        return Ok(auth);
    };
    let SorobanAddressCredentials { nonce, .. } = credentials;

    let preimage = HashIdPreimage::SorobanAuthorization(HashIdPreimageSorobanAuthorization {
        network_id: network_id.clone(),
        invocation: auth.root_invocation.clone(),
        nonce: *nonce,
        signature_expiration_ledger,
    })
    .to_xdr(Limits::none())?;

    let payload = Sha256::digest(preimage);
    let signature = signer.sign(&payload);

    let map = ScMap::sorted_from(vec![
        (
            ScVal::Symbol(ScSymbol("public_key".try_into()?)),
            ScVal::Bytes(
                signer
                    .verifying_key()
                    .to_bytes()
                    .to_vec()
                    .try_into()
                    .map_err(Error::Xdr)?,
            ),
        ),
        (
            ScVal::Symbol(ScSymbol("signature".try_into()?)),
            ScVal::Bytes(
                signature
                    .to_bytes()
                    .to_vec()
                    .try_into()
                    .map_err(Error::Xdr)?,
            ),
        ),
    ])
    .map_err(Error::Xdr)?;
    credentials.signature = ScVal::Vec(Some(
        vec![ScVal::Map(Some(map))].try_into().map_err(Error::Xdr)?,
    ));
    credentials.signature_expiration_ledger = signature_expiration_ledger;
    auth.credentials = SorobanCredentials::Address(credentials.clone());
    Ok(auth)
}

pub struct Signer {
    pub kind: SignerKind,
    pub print: Print,
}

#[allow(clippy::module_name_repetitions, clippy::large_enum_variant)]
pub enum SignerKind {
    Local(LocalKey),
    Lab,
    SecureStore(SecureStoreEntry),
}

impl Signer {
    pub fn sign_tx(
        &self,
        tx: Transaction,
        network: &Network,
    ) -> Result<TransactionEnvelope, Error> {
        let tx_env = TransactionEnvelope::Tx(TransactionV1Envelope {
            tx,
            signatures: VecM::default(),
        });
        self.sign_tx_env(&tx_env, network)
    }

    pub fn sign_tx_env(
        &self,
        tx_env: &TransactionEnvelope,
        network: &Network,
    ) -> Result<TransactionEnvelope, Error> {
        match &tx_env {
            TransactionEnvelope::Tx(TransactionV1Envelope { tx, signatures }) => {
                let tx_hash = transaction_hash(tx, &network.network_passphrase)?;
                self.print
                    .infoln(format!("Signing transaction: {}", hex::encode(tx_hash),));
                let decorated_signature = match &self.kind {
                    SignerKind::Local(key) => key.sign_tx_hash(tx_hash)?,
                    SignerKind::Lab => Lab::sign_tx_env(tx_env, network, &self.print)?,
                    SignerKind::SecureStore(entry) => entry.sign_tx_hash(tx_hash)?,
                };
                let mut sigs = signatures.clone().into_vec();
                sigs.push(decorated_signature);
                Ok(TransactionEnvelope::Tx(TransactionV1Envelope {
                    tx: tx.clone(),
                    signatures: sigs.try_into()?,
                }))
            }
            _ => Err(Error::UnsupportedTransactionEnvelopeType),
        }
    }
}

pub struct LocalKey {
    pub key: ed25519_dalek::SigningKey,
}

impl LocalKey {
    pub fn sign_tx_hash(&self, tx_hash: [u8; 32]) -> Result<DecoratedSignature, Error> {
        let hint = SignatureHint(self.key.verifying_key().to_bytes()[28..].try_into()?);
        let signature = Signature(self.key.sign(&tx_hash).to_bytes().to_vec().try_into()?);
        Ok(DecoratedSignature { hint, signature })
    }
}

pub struct Lab;

impl Lab {
    const URL: &str = "https://lab.stellar.org/transaction/cli-sign";

    pub fn sign_tx_env(
        tx_env: &TransactionEnvelope,
        network: &Network,
        printer: &Print,
    ) -> Result<DecoratedSignature, Error> {
        let xdr = tx_env.to_xdr_base64(Limits::none())?;

        let mut url = url::Url::parse(Self::URL)?;
        url.query_pairs_mut()
            .append_pair("networkPassphrase", &network.network_passphrase)
            .append_pair("xdr", &xdr);
        let url = url.to_string();

        printer.globeln(format!("Opening lab to sign transaction: {url}"));
        open::that(url)?;

        Err(Error::ReturningSignatureFromLab)
    }
}

pub struct SecureStoreEntry {
    pub name: String,
    pub hd_path: Option<usize>,
}

impl SecureStoreEntry {
    pub fn sign_tx_hash(&self, tx_hash: [u8; 32]) -> Result<DecoratedSignature, Error> {
        let entry = StellarEntry::new(&self.name)?;
        let hint = SignatureHint(entry.get_public_key(self.hd_path)?.0[28..].try_into()?);
        let signed_tx_hash = entry.sign_data(&tx_hash, self.hd_path)?;
        let signature = Signature(signed_tx_hash.clone().try_into()?);
        Ok(DecoratedSignature { hint, signature })
    }
}
