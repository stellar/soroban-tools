use stellar_ledger::Blob;

use soroban_test::{AssertExt, TestEnv};
use std::sync::Arc;

use stellar_ledger::emulator_test_support::*;

use soroban_cli::{
    tx::builder::TxExt,
    xdr::{self, Limits, OperationBody, ReadXdr, TransactionEnvelope, WriteXdr},
};

use test_case::test_case;

#[test_case("nanos", 0; "when the device is NanoS")]
#[test_case("nanox", 1; "when the device is NanoX")]
#[test_case("nanosp", 2; "when the device is NanoS Plus")]
#[tokio::test]
async fn test_signer(ledger_device_model: &str, hd_path: u32) {
    let sandbox = Arc::new(TestEnv::new());
    let container = TestEnv::speculos_container(ledger_device_model).await;
    let host_port = container.get_host_port_ipv4(9998).await.unwrap();
    let ui_host_port = container.get_host_port_ipv4(5000).await.unwrap();

    let ledger = ledger(host_port).await;

    let key = ledger.get_public_key(&hd_path.into()).await.unwrap();

    let verifying_key = ed25519_dalek::VerifyingKey::from_bytes(&key.0).unwrap();
    let body: OperationBody =
        (&soroban_cli::commands::tx::new::bump_sequence::Args { bump_to: 100 }).into();
    let operation = xdr::Operation {
        body,
        source_account: None,
    };
    let source_account = xdr::MuxedAccount::Ed25519(key.0.into());
    let tx_env: TransactionEnvelope =
        xdr::Transaction::new_tx(source_account, 100, 100, operation).into();
    let tx_env = tx_env.to_xdr_base64(Limits::none()).unwrap();

    let hash: xdr::Hash = sandbox
        .new_assert_cmd("tx")
        .arg("hash")
        .write_stdin(tx_env.as_bytes())
        .assert()
        .success()
        .stdout_as_str()
        .parse()
        .unwrap();

    let sign = tokio::task::spawn_blocking({
        let sandbox = Arc::clone(&sandbox);

        move || {
            sandbox
                .new_assert_cmd("tx")
                .arg("sign")
                .arg("--sign-with-ledger")
                .arg("--hd-path")
                .arg(hd_path.to_string())
                .write_stdin(tx_env.as_bytes())
                .env("SPECULOS_PORT", host_port.to_string())
                .env("RUST_LOGS", "trace")
                .assert()
                .success()
                .stdout_as_str()
        }
    });

    let approve = tokio::task::spawn(approve_tx_hash_signature(
        ui_host_port,
        ledger_device_model.to_string(),
    ));

    let response = sign.await.unwrap();
    approve.await.unwrap();
    let txn_env =
        xdr::TransactionEnvelope::from_xdr_base64(&response, xdr::Limits::none()).unwrap();
    let xdr::TransactionEnvelope::Tx(tx_env) = txn_env else {
        panic!("expected Tx")
    };
    let signatures = tx_env.signatures.to_vec();
    let signature = signatures[0].signature.to_vec();
    verifying_key
        .verify_strict(
            &hash.0,
            &ed25519_dalek::Signature::from_slice(&signature).unwrap(),
        )
        .unwrap();
}
