use hd_path::HdPath;
use ledger_transport::APDUCommand;
pub use ledger_transport::Exchange;

use ledger_transport_hid::{
    hidapi::{HidApi, HidError},
    LedgerHIDError,
};

pub use ledger_transport_hid::TransportNativeHID;

use std::vec;
use stellar_strkey::DecodeError;
use stellar_xdr::curr::{
    self as xdr, Hash, Limits, Transaction, TransactionSignaturePayload,
    TransactionSignaturePayloadTaggedTransaction, WriteXdr,
};

pub use crate::signer::Blob;
pub mod hd_path;
mod signer;

pub mod emulator_test_support;

// this is from https://github.com/LedgerHQ/ledger-live/blob/36cfbf3fa3300fd99bcee2ab72e1fd8f280e6280/libs/ledgerjs/packages/hw-app-str/src/Str.ts#L181
const APDU_MAX_SIZE: u8 = 150;
const HD_PATH_ELEMENTS_COUNT: u8 = 3;
const BUFFER_SIZE: u8 = 1 + HD_PATH_ELEMENTS_COUNT * 4;
const CHUNK_SIZE: u8 = APDU_MAX_SIZE - BUFFER_SIZE;

// These constant values are from https://github.com/LedgerHQ/app-stellar/blob/develop/docs/COMMANDS.md
const SIGN_TX_RESPONSE_SIZE: usize = 64;

const CLA: u8 = 0xE0;

const GET_PUBLIC_KEY: u8 = 0x02;
const P1_GET_PUBLIC_KEY: u8 = 0x00;
const P2_GET_PUBLIC_KEY_NO_DISPLAY: u8 = 0x00;
const P2_GET_PUBLIC_KEY_DISPLAY: u8 = 0x01;

const SIGN_TX: u8 = 0x04;
const P1_SIGN_TX_FIRST: u8 = 0x00;
const P1_SIGN_TX_NOT_FIRST: u8 = 0x80;
const P2_SIGN_TX_LAST: u8 = 0x00;
const P2_SIGN_TX_MORE: u8 = 0x80;

const GET_APP_CONFIGURATION: u8 = 0x06;
const P1_GET_APP_CONFIGURATION: u8 = 0x00;
const P2_GET_APP_CONFIGURATION: u8 = 0x00;

const SIGN_TX_HASH: u8 = 0x08;
const P1_SIGN_TX_HASH: u8 = 0x00;
const P2_SIGN_TX_HASH: u8 = 0x00;

const RETURN_CODE_OK: u16 = 36864; // APDUAnswer.retcode which means success from Ledger

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Error occurred while initializing HIDAPI: {0}")]
    HidApiError(#[from] HidError),

    #[error("Error occurred while initializing Ledger HID transport: {0}")]
    LedgerHidError(#[from] LedgerHIDError),

    #[error("Error with ADPU exchange with Ledger device: {0}")]
    APDUExchangeError(String),

    #[error("Error occurred while exchanging with Ledger device: {0}")]
    LedgerConnectionError(String),

    #[error("Error occurred while parsing BIP32 path: {0}")]
    Bip32PathError(String),

    #[error(transparent)]
    XdrError(#[from] xdr::Error),

    #[error(transparent)]
    DecodeError(#[from] DecodeError),
}

pub struct LedgerSigner<T: Exchange> {
    transport: T,
}

unsafe impl<T> Send for LedgerSigner<T> where T: Exchange {}
unsafe impl<T> Sync for LedgerSigner<T> where T: Exchange {}

/// # Errors
/// Could fail to make the connection to the Ledger device
pub fn native() -> Result<LedgerSigner<TransportNativeHID>, Error> {
    Ok(LedgerSigner {
        transport: get_transport()?,
    })
}

impl<T> LedgerSigner<T>
where
    T: Exchange,
{
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    /// # Errors
    /// Returns an error if there is an issue with connecting with the device
    pub fn native() -> Result<LedgerSigner<TransportNativeHID>, Error> {
        Ok(LedgerSigner {
            transport: get_transport()?,
        })
    }
    /// Get the device app's configuration
    /// # Errors
    /// Returns an error if there is an issue with connecting with the device or getting the config from the device
    pub async fn get_app_configuration(&self) -> Result<Vec<u8>, Error> {
        let command = APDUCommand {
            cla: CLA,
            ins: GET_APP_CONFIGURATION,
            p1: P1_GET_APP_CONFIGURATION,
            p2: P2_GET_APP_CONFIGURATION,
            data: vec![],
        };
        self.send_command_to_ledger(command).await
    }

    /// Sign a Stellar transaction hash with the account on the Ledger device
    /// based on impl from [https://github.com/LedgerHQ/ledger-live/blob/develop/libs/ledgerjs/packages/hw-app-str/src/Str.ts#L166](https://github.com/LedgerHQ/ledger-live/blob/develop/libs/ledgerjs/packages/hw-app-str/src/Str.ts#L166)
    /// # Errors
    /// Returns an error if there is an issue with connecting with the device or signing the given tx on the device. Or, if the device has not enabled hash signing
    pub async fn sign_transaction_hash(
        &self,
        hd_path: impl Into<HdPath>,
        transaction_hash: &[u8; 32],
    ) -> Result<Vec<u8>, Error> {
        self.sign_blob(&hd_path.into(), transaction_hash).await
    }

    /// Sign a Stellar transaction with the account on the Ledger device
    /// # Errors
    /// Returns an error if there is an issue with connecting with the device or signing the given tx on the device
    #[allow(clippy::missing_panics_doc)]
    pub async fn sign_transaction(
        &self,
        hd_path: impl Into<HdPath>,
        transaction: Transaction,
        network_id: Hash,
    ) -> Result<Vec<u8>, Error> {
        let tagged_transaction = TransactionSignaturePayloadTaggedTransaction::Tx(transaction);
        let signature_payload = TransactionSignaturePayload {
            network_id,
            tagged_transaction,
        };
        let mut signature_payload_as_bytes = signature_payload.to_xdr(Limits::none())?;

        let mut hd_path_to_bytes = hd_path.into().to_vec()?;

        let capacity = 1 + hd_path_to_bytes.len() + signature_payload_as_bytes.len();
        let mut data: Vec<u8> = Vec::with_capacity(capacity);

        data.insert(0, HD_PATH_ELEMENTS_COUNT);
        data.append(&mut hd_path_to_bytes);
        data.append(&mut signature_payload_as_bytes);

        let chunks = data.chunks(CHUNK_SIZE as usize);
        let chunks_count = chunks.len();

        let mut result = Vec::with_capacity(SIGN_TX_RESPONSE_SIZE);
        for (i, chunk) in chunks.enumerate() {
            let is_first_chunk = i == 0;
            let is_last_chunk = chunks_count == i + 1;

            let command = APDUCommand {
                cla: CLA,
                ins: SIGN_TX,
                p1: if is_first_chunk {
                    P1_SIGN_TX_FIRST
                } else {
                    P1_SIGN_TX_NOT_FIRST
                },
                p2: if is_last_chunk {
                    P2_SIGN_TX_LAST
                } else {
                    P2_SIGN_TX_MORE
                },
                data: chunk.to_vec(),
            };

            let mut r = self.send_command_to_ledger(command).await?;
            result.append(&mut r);
        }

        Ok(result)
    }

    /// The `display_and_confirm` bool determines if the Ledger will display the public key on its screen and requires user approval to share
    async fn get_public_key_with_display_flag(
        &self,
        hd_path: impl Into<HdPath>,
        display_and_confirm: bool,
    ) -> Result<stellar_strkey::ed25519::PublicKey, Error> {
        // convert the hd_path into bytes to be sent as `data` to the Ledger
        // the first element of the data should be the number of elements in the path
        let hd_path = hd_path.into();
        let hd_path_elements_count = hd_path.depth();
        let mut hd_path_to_bytes = hd_path.to_vec()?;
        hd_path_to_bytes.insert(0, hd_path_elements_count);

        let p2 = if display_and_confirm {
            P2_GET_PUBLIC_KEY_DISPLAY
        } else {
            P2_GET_PUBLIC_KEY_NO_DISPLAY
        };

        // more information about how to build this command can be found at https://github.com/LedgerHQ/app-stellar/blob/develop/docs/COMMANDS.md
        let command = APDUCommand {
            cla: CLA,
            ins: GET_PUBLIC_KEY,
            p1: P1_GET_PUBLIC_KEY,
            p2,
            data: hd_path_to_bytes,
        };

        tracing::info!("APDU in: {}", hex::encode(command.serialize()));

        self.send_command_to_ledger(command)
            .await
            .and_then(|p| Ok(stellar_strkey::ed25519::PublicKey::from_payload(&p)?))
    }

    async fn send_command_to_ledger(
        &self,
        command: APDUCommand<Vec<u8>>,
    ) -> Result<Vec<u8>, Error> {
        match self.transport.exchange(&command).await {
            Ok(response) => {
                tracing::info!(
                    "APDU out: {}\nAPDU ret code: {:x}",
                    hex::encode(response.apdu_data()),
                    response.retcode(),
                );
                // Ok means we successfully connected with the Ledger but it doesn't mean our request succeeded. We still need to check the response.retcode
                if response.retcode() == RETURN_CODE_OK {
                    return Ok(response.data().to_vec());
                }

                let retcode = response.retcode();
                let error_string = format!("Ledger APDU retcode: 0x{retcode:X}");
                Err(Error::APDUExchangeError(error_string))
            }
            Err(_err) => Err(Error::LedgerConnectionError(
                "Error connecting to ledger device".to_string(),
            )),
        }
    }
}

#[async_trait::async_trait]
impl<T> Blob for LedgerSigner<T>
where
    T: Exchange,
{
    type Key = HdPath;
    type Error = Error;
    /// Get the public key from the device
    /// # Errors
    /// Returns an error if there is an issue with connecting with the device or getting the public key from the device
    async fn get_public_key(
        &self,
        index: &Self::Key,
    ) -> Result<stellar_strkey::ed25519::PublicKey, Error> {
        self.get_public_key_with_display_flag(*index, false).await
    }

    /// Sign a blob of data with the account on the Ledger device
    /// based on impl from [https://github.com/LedgerHQ/ledger-live/blob/develop/libs/ledgerjs/packages/hw-app-str/src/Str.ts#L166](https://github.com/LedgerHQ/ledger-live/blob/develop/libs/ledgerjs/packages/hw-app-str/src/Str.ts#L166)
    /// # Errors
    /// Returns an error if there is an issue with connecting with the device or signing the given tx on the device. Or, if the device has not enabled hash signing
    async fn sign_blob(&self, index: &Self::Key, blob: &[u8]) -> Result<Vec<u8>, Error> {
        let mut hd_path_to_bytes = index.to_vec()?;

        let capacity = 1 + hd_path_to_bytes.len() + blob.len();
        let mut data: Vec<u8> = Vec::with_capacity(capacity);

        data.insert(0, HD_PATH_ELEMENTS_COUNT);
        data.append(&mut hd_path_to_bytes);
        data.extend_from_slice(blob);

        let command = APDUCommand {
            cla: CLA,
            ins: SIGN_TX_HASH,
            p1: P1_SIGN_TX_HASH,
            p2: P2_SIGN_TX_HASH,
            data,
        };

        self.send_command_to_ledger(command).await
    }
}

fn get_transport() -> Result<TransportNativeHID, Error> {
    // instantiate the connection to Ledger, this will return an error if Ledger is not connected
    let hidapi = HidApi::new().map_err(Error::HidApiError)?;
    TransportNativeHID::new(&hidapi).map_err(Error::LedgerHidError)
}

pub const TEST_NETWORK_PASSPHRASE: &[u8] = b"Test SDF Network ; September 2015";
#[cfg(test)]
pub fn test_network_hash() -> Hash {
    use sha2::Digest;
    Hash(sha2::Sha256::digest(TEST_NETWORK_PASSPHRASE).into())
}

#[cfg(all(test, feature = "http-transport"))]
mod test {
    use httpmock::prelude::*;
    use serde_json::json;

    use super::emulator_test_support::http_transport::Emulator;
    use crate::Blob;

    use std::vec;

    use super::xdr::{self, Operation, OperationBody, Transaction, Uint256};

    use crate::{test_network_hash, Error, LedgerSigner};

    use stellar_xdr::curr::{
        Memo, MuxedAccount, PaymentOp, Preconditions, SequenceNumber, TransactionExt,
    };

    fn ledger(server: &MockServer) -> LedgerSigner<Emulator> {
        let transport = Emulator::new(&server.host(), server.port());
        LedgerSigner::new(transport)
    }

    #[tokio::test]
    async fn test_get_public_key() {
        let server = MockServer::start();
        let mock_server = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("accept", "application/json")
                .header("content-type", "application/json")
                .json_body(json!({ "apduHex": "e00200000d038000002c8000009480000000" }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({"data": "e93388bbfd2fbd11806dd0bd59cea9079e7cc70ce7b1e154f114cdfe4e466ecd9000"}));
        });
        let ledger = ledger(&server);
        let public_key = ledger.get_public_key(&0u32.into()).await.unwrap();
        let public_key_string = public_key.to_string();
        let expected_public_key = "GDUTHCF37UX32EMANXIL2WOOVEDZ47GHBTT3DYKU6EKM37SOIZXM2FN7";
        assert_eq!(public_key_string, expected_public_key);

        mock_server.assert();
    }

    #[tokio::test]
    async fn test_get_app_configuration() {
        let server = MockServer::start();
        let mock_server = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("accept", "application/json")
                .header("content-type", "application/json")
                .json_body(json!({ "apduHex": "e006000000" }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({"data": "000500039000"}));
        });
        let ledger = ledger(&server);
        let config = ledger.get_app_configuration().await.unwrap();
        assert_eq!(config, vec![0, 5, 0, 3]);

        mock_server.assert();
    }

    #[tokio::test]
    async fn test_sign_tx() {
        let server = MockServer::start();
        let mock_request_1 = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("accept", "application/json")
                .header("content-type", "application/json")
                .json_body(json!({ "apduHex": "e004008089038000002c8000009480000000cee0302d59844d32bdca915c8203dd44b33fbb7edc19051ea37abedf28ecd472000000020000000000000000000000000000000000000000000000000000000000000000000000000000006400000000000000010000000000000001000000075374656c6c6172000000000100000001000000000000000000000000" }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({"data": "9000"}));
        });

        let mock_request_2 = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("accept", "application/json")
                .header("content-type", "application/json")
                .json_body(json!({ "apduHex": "e0048000500000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000006400000000" }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({"data": "5c2f8eb41e11ab922800071990a25cf9713cc6e7c43e50e0780ddc4c0c6da50c784609ef14c528a12f520d8ea9343b49083f59c51e3f28af8c62b3edeaade60e9000"}));
        });

        let ledger = ledger(&server);

        let fake_source_acct = [0; 32];
        let fake_dest_acct = [0; 32];
        let tx = Transaction {
            source_account: MuxedAccount::Ed25519(Uint256(fake_source_acct)),
            fee: 100,
            seq_num: SequenceNumber(1),
            cond: Preconditions::None,
            memo: Memo::Text("Stellar".as_bytes().try_into().unwrap()),
            ext: TransactionExt::V0,
            operations: [Operation {
                source_account: Some(MuxedAccount::Ed25519(Uint256(fake_source_acct))),
                body: OperationBody::Payment(PaymentOp {
                    destination: MuxedAccount::Ed25519(Uint256(fake_dest_acct)),
                    asset: xdr::Asset::Native,
                    amount: 100,
                }),
            }]
            .try_into()
            .unwrap(),
        };

        let response = ledger
            .sign_transaction(0, tx, test_network_hash())
            .await
            .unwrap();
        assert_eq!(
            hex::encode(response),
            "5c2f8eb41e11ab922800071990a25cf9713cc6e7c43e50e0780ddc4c0c6da50c784609ef14c528a12f520d8ea9343b49083f59c51e3f28af8c62b3edeaade60e"
        );

        mock_request_1.assert();
        mock_request_2.assert();
    }

    #[tokio::test]
    async fn test_sign_tx_hash_when_hash_signing_is_not_enabled() {
        let server = MockServer::start();
        let mock_server = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("accept", "application/json")
                .header("content-type", "application/json")
                .json_body(json!({ "apduHex": "e00800004d038000002c800000948000000033333839653966306631613635663139373336636163663534346332653832353331336538343437663536393233336262386462333961613630376338383839" }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({"data": "6c66"}));
        });

        let ledger = ledger(&server);
        let path = 0;
        let test_hash = b"3389e9f0f1a65f19736cacf544c2e825313e8447f569233bb8db39aa607c8889";

        let err = ledger.sign_blob(&path.into(), test_hash).await.unwrap_err();
        if let Error::APDUExchangeError(msg) = err {
            assert_eq!(msg, "Ledger APDU retcode: 0x6C66");
        } else {
            panic!("Unexpected error: {err:?}");
        }

        mock_server.assert();
    }

    #[tokio::test]
    async fn test_sign_tx_hash_when_hash_signing_is_enabled() {
        let server = MockServer::start();
        let mock_server = server.mock(|when, then| {
            when.method(POST)
                .path("/")
                .header("accept", "application/json")
                .header("content-type", "application/json")
                .json_body(json!({ "apduHex": "e00800002d038000002c80000094800000003389e9f0f1a65f19736cacf544c2e825313e8447f569233bb8db39aa607c8889" }));
            then.status(200)
                .header("content-type", "application/json")
                .json_body(json!({"data": "6970b9c9d3a6f4de7fb93e8d3920ec704fc4fece411873c40570015bbb1a60a197622bc3bf5644bb38ae73e1b96e4d487d716d142d46c7e944f008dece92df079000"}));
        });

        let ledger = ledger(&server);
        let path = 0;
        let mut test_hash = vec![0u8; 32];

        hex::decode_to_slice(
            "3389e9f0f1a65f19736cacf544c2e825313e8447f569233bb8db39aa607c8889",
            &mut test_hash as &mut [u8],
        )
        .unwrap();

        let response = ledger.sign_blob(&path.into(), &test_hash).await.unwrap();

        assert_eq!(
            hex::encode(response),
            "6970b9c9d3a6f4de7fb93e8d3920ec704fc4fece411873c40570015bbb1a60a197622bc3bf5644bb38ae73e1b96e4d487d716d142d46c7e944f008dece92df07"
        );

        mock_server.assert();
    }
}
