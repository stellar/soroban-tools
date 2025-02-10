use stellar_ledger::hd_path::HdPath;
use stellar_ledger::{Blob, Error};

use std::sync::Arc;

use stellar_xdr::curr::{
    self as xdr, Memo, MuxedAccount, Operation, OperationBody, PaymentOp, Preconditions,
    SequenceNumber, Transaction, TransactionExt, Uint256,
};

use stellar_ledger::emulator_test_support::*;

use test_case::test_case;

#[test_case("nanos"; "when the device is NanoS")]
#[test_case("nanox"; "when the device is NanoX")]
#[test_case("nanosp"; "when the device is NanoS Plus")]
#[tokio::test]
async fn test_get_public_key(ledger_device_model: &str) {
    let container = get_container(ledger_device_model).await;
    let host_port = container.get_host_port_ipv4(9998).await.unwrap();
    let ui_host_port: u16 = container.get_host_port_ipv4(5000).await.unwrap();
    wait_for_emulator_start_text(ui_host_port).await;

    let ledger = ledger(host_port).await;

    match ledger.get_public_key(&0.into()).await {
        Ok(public_key) => {
            let public_key_string = public_key.to_string();
            // This is determined by the seed phrase used to start up the emulator
            // TODO: make the seed phrase configurable
            let expected_public_key = "GDUTHCF37UX32EMANXIL2WOOVEDZ47GHBTT3DYKU6EKM37SOIZXM2FN7";
            assert_eq!(public_key_string, expected_public_key);
        }
        Err(e) => {
            println!("{e}");
            assert!(false);
        }
    }
}

#[test_case("nanos"; "when the device is NanoS")]
#[test_case("nanox"; "when the device is NanoX")]
#[test_case("nanosp"; "when the device is NanoS Plus")]
#[tokio::test]
async fn test_get_app_configuration(ledger_device_model: &str) {
    let container = get_container(ledger_device_model).await;
    let host_port = container.get_host_port_ipv4(9998).await.unwrap();
    let ui_host_port: u16 = container.get_host_port_ipv4(5000).await.unwrap();
    wait_for_emulator_start_text(ui_host_port).await;

    let ledger = ledger(host_port).await;

    match ledger.get_app_configuration().await {
        Ok(config) => {
            assert_eq!(config, vec![0, 5, 0, 3]);
        }
        Err(e) => {
            println!("{e}");
            assert!(false);
        }
    };
}

#[test_case("nanos"; "when the device is NanoS")]
#[test_case("nanox"; "when the device is NanoX")]
#[test_case("nanosp"; "when the device is NanoS Plus")]
#[tokio::test]
async fn test_sign_tx(ledger_device_model: &str) {
    let container = get_container(ledger_device_model).await;
    let host_port = container.get_host_port_ipv4(9998).await.unwrap();
    let ui_host_port: u16 = container.get_host_port_ipv4(5000).await.unwrap();
    wait_for_emulator_start_text(ui_host_port).await;

    let ledger = Arc::new(ledger(host_port).await);

    let path = HdPath(0);

    let source_account_str = "GAQNVGMLOXSCWH37QXIHLQJH6WZENXYSVWLPAEF4673W64VRNZLRHMFM";
    let source_account_bytes = match stellar_strkey::Strkey::from_string(source_account_str) {
        Ok(key) => match key {
            stellar_strkey::Strkey::PublicKeyEd25519(p) => p.0,
            _ => {
                eprintln!("Error decoding public key: {:?}", key);
                return;
            }
        },
        Err(err) => {
            eprintln!("Error decoding public key: {}", err);
            return;
        }
    };

    let destination_account_str = "GCKUD4BHIYSAYHU7HBB5FDSW6CSYH3GSOUBPWD2KE7KNBERP4BSKEJDV";
    let destination_account_bytes =
        match stellar_strkey::Strkey::from_string(destination_account_str) {
            Ok(key) => match key {
                stellar_strkey::Strkey::PublicKeyEd25519(p) => p.0,
                _ => {
                    eprintln!("Error decoding public key: {:?}", key);
                    return;
                }
            },
            Err(err) => {
                eprintln!("Error decoding public key: {}", err);
                return;
            }
        };

    let tx = Transaction {
        source_account: MuxedAccount::Ed25519(Uint256(source_account_bytes)),
        fee: 100,
        seq_num: SequenceNumber(1),
        cond: Preconditions::None,
        memo: Memo::Text("Stellar".try_into().unwrap()),
        ext: TransactionExt::V0,
        operations: [Operation {
            source_account: Some(MuxedAccount::Ed25519(Uint256(source_account_bytes))),
            body: OperationBody::Payment(PaymentOp {
                destination: MuxedAccount::Ed25519(Uint256(destination_account_bytes)),
                asset: xdr::Asset::Native,
                amount: 100,
            }),
        }]
        .try_into()
        .unwrap(),
    };

    let sign = tokio::task::spawn({
        let ledger = Arc::clone(&ledger);
        async move { ledger.sign_transaction(path, tx, test_network_hash()).await }
    });
    let approve = tokio::task::spawn(approve_tx_signature(
        ui_host_port,
        ledger_device_model.to_string(),
    ));

    let result = sign.await.unwrap();
    let _ = approve.await.unwrap();

    match result {
        Ok(response) => {
            assert_eq!( hex::encode(response), "5c2f8eb41e11ab922800071990a25cf9713cc6e7c43e50e0780ddc4c0c6da50c784609ef14c528a12f520d8ea9343b49083f59c51e3f28af8c62b3edeaade60e");
        }
        Err(e) => {
            println!("{e}");
            assert!(false);
        }
    };
}

#[test_case("nanos"; "when the device is NanoS")]
#[test_case("nanox"; "when the device is NanoX")]
#[test_case("nanosp"; "when the device is NanoS Plus")]
#[tokio::test]
async fn test_sign_tx_hash_when_hash_signing_is_not_enabled(ledger_device_model: &str) {
    let container = get_container(ledger_device_model).await;
    let host_port = container.get_host_port_ipv4(9998).await.unwrap();
    let ui_host_port: u16 = container.get_host_port_ipv4(5000).await.unwrap();
    wait_for_emulator_start_text(ui_host_port).await;

    let ledger = ledger(host_port).await;

    let path = 0;
    let test_hash = b"313e8447f569233bb8db39aa607c8889";

    let result = ledger.sign_transaction_hash(path, test_hash).await;
    if let Err(Error::APDUExchangeError(msg)) = result {
        assert_eq!(msg, "Ledger APDU retcode: 0x6C66");
        // this error code is SW_TX_HASH_SIGNING_MODE_NOT_ENABLED https://github.com/LedgerHQ/app-stellar/blob/develop/docs/COMMANDS.md
    } else {
        panic!("Unexpected result: {:?}", result);
    }
}

#[test_case("nanos"; "when the device is NanoS")]
#[test_case("nanox"; "when the device is NanoX")]
#[test_case("nanosp"; "when the device is NanoS Plus")]
#[tokio::test]
async fn test_sign_tx_hash_when_hash_signing_is_enabled(ledger_device_model: &str) {
    let container = get_container(ledger_device_model).await;
    let host_port = container.get_host_port_ipv4(9998).await.unwrap();
    let ui_host_port: u16 = container.get_host_port_ipv4(5000).await.unwrap();

    wait_for_emulator_start_text(ui_host_port).await;
    enable_hash_signing(ui_host_port).await;

    let ledger = Arc::new(ledger(host_port).await);

    let path = 0;
    let mut test_hash = [0u8; 32];

    match hex::decode_to_slice(
        "313e8447f569233bb8db39aa607c8889313e8447f569233bb8db39aa607c8889",
        &mut test_hash as &mut [u8],
    ) {
        Ok(()) => {}
        Err(e) => {
            panic!("Unexpected result: {e}");
        }
    }

    let sign = tokio::task::spawn({
        let ledger = Arc::clone(&ledger);
        async move { ledger.sign_transaction_hash(path, &test_hash).await }
    });
    let approve = tokio::task::spawn(approve_tx_hash_signature(
        ui_host_port,
        ledger_device_model.to_string(),
    ));

    let response = sign.await.unwrap();
    let _ = approve.await.unwrap();

    match response {
        Ok(response) => {
            assert_eq!( hex::encode(response), "e0fa9d19f34ddd494bbb794645fc82eb5ebab29e74160f1b1d5697e749aada7c6b367236df87326b0fdc921ed39702242fc8b14414f4e0ee3e775f1fd0208101");
        }
        Err(e) => {
            panic!("Unexpected result: {e}");
        }
    }
}
