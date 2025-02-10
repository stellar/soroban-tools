// This is based on the `ledger-transport-zemu` crate's TransportZemuHttp: https://github.com/Zondax/ledger-rs/tree/master/ledger-transport-zemu
// Instead of using TransportZemuHttp mod from the crate, we are including a custom copy here for a couple of reasons:
// - we get more control over the mod for our testing purposes
// - the ledger-transport-zemu TransportZemuHttp includes a Grpc implementation that we don't need right now, and was causing some errors with dependency mismatches when trying to use the whole TransportZemuHttp mod.

use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, CONTENT_TYPE};
use reqwest::{Client as HttpClient, Response};
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use std::time::Duration;

use ledger_transport::{async_trait, APDUAnswer, APDUCommand, Exchange};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum LedgerZemuError {
    /// zemu reponse error
    #[error("Zemu response error")]
    ResponseError,
    /// Inner error
    #[error("Ledger inner error")]
    InnerError,
}

pub struct Emulator {
    url: String,
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct ZemuRequest {
    apdu_hex: String,
}

#[derive(Deserialize, Debug, Clone)]
struct ZemuResponse {
    data: String,
    error: Option<String>,
}

impl Emulator {
    #[allow(dead_code)] //this is being used in tests only
    #[must_use]
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            url: format!("http://{host}:{port}"),
        }
    }
}

#[async_trait]
impl Exchange for Emulator {
    type Error = LedgerZemuError;
    type AnswerType = Vec<u8>;

    async fn exchange<I>(
        &self,
        command: &APDUCommand<I>,
    ) -> Result<APDUAnswer<Self::AnswerType>, Self::Error>
    where
        I: Deref<Target = [u8]> + Send + Sync,
    {
        let raw_command = hex::encode(command.serialize());
        let request = ZemuRequest {
            apdu_hex: raw_command,
        };

        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let resp: Response = HttpClient::new()
            .post(&self.url)
            .headers(headers)
            .timeout(Duration::from_secs(60))
            .json(&request)
            .send()
            .await
            .map_err(|e| {
                tracing::error!("create http client error: {:?}", e);
                LedgerZemuError::InnerError
            })?;
        tracing::debug!("http response: {:?}", resp);

        if resp.status().is_success() {
            let result: ZemuResponse = resp.json().await.map_err(|e| {
                tracing::error!("error response: {:?}", e);
                LedgerZemuError::ResponseError
            })?;
            if result.error.is_none() {
                APDUAnswer::from_answer(hex::decode(result.data).expect("decode error"))
                    .map_err(|_| LedgerZemuError::ResponseError)
            } else {
                Err(LedgerZemuError::ResponseError)
            }
        } else {
            tracing::error!("error response: {:?}", resp.status());
            Err(LedgerZemuError::ResponseError)
        }
    }
}
