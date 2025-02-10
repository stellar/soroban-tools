use soroban_cli::assembled::simulate_and_assemble_transaction;
use soroban_cli::xdr::{Limits, ReadXdr, TransactionEnvelope, WriteXdr};
use soroban_test::{AssertExt, TestEnv};

use crate::integration::util::{deploy_contract, DeployKind, DeployOptions, HELLO_WORLD};

pub mod operations;

#[tokio::test]
async fn simulate() {
    let sandbox = &TestEnv::new();
    let salt = Some(String::from("A"));
    let xdr_base64_build_only = deploy_contract(
        sandbox,
        HELLO_WORLD,
        DeployOptions {
            kind: DeployKind::BuildOnly,
            salt: salt.clone(),
            ..Default::default()
        },
    )
    .await;
    let xdr_base64_sim_only = deploy_contract(
        sandbox,
        HELLO_WORLD,
        DeployOptions {
            kind: DeployKind::SimOnly,
            salt: salt.clone(),
            ..Default::default()
        },
    )
    .await;
    let tx_env =
        TransactionEnvelope::from_xdr_base64(&xdr_base64_build_only, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env.clone()).unwrap();
    let assembled_str = sandbox
        .new_assert_cmd("tx")
        .arg("simulate")
        .write_stdin(xdr_base64_build_only.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    let tx_env_from_cli_tx =
        TransactionEnvelope::from_xdr_base64(&assembled_str, Limits::none()).unwrap();
    let tx_env_sim_only =
        TransactionEnvelope::from_xdr_base64(&xdr_base64_sim_only, Limits::none()).unwrap();
    assert_eq!(tx_env_from_cli_tx, tx_env_sim_only);
    assert_eq!(xdr_base64_sim_only, assembled_str);
    let assembled = simulate_and_assemble_transaction(&sandbox.client(), &tx)
        .await
        .unwrap();
    let txn_env: TransactionEnvelope = assembled.transaction().clone().into();
    assert_eq!(
        txn_env.to_xdr_base64(Limits::none()).unwrap(),
        assembled_str
    );
}

#[tokio::test]
async fn txn_hash() {
    let sandbox = &TestEnv::new();

    let xdr_base64 = "AAAAAgAAAACVk/0xt9tV/cUbF53iwQ3tkKLlq9zG2wV5qd9lRjZjlQAHt/sAFsKTAAAABAAAAAEAAAAAAAAAAAAAAABmOg6nAAAAAAAAAAEAAAAAAAAAGAAAAAAAAAABfcHs35M1GZ/JkY2+DHMs4dEUaqjynMnDYK/Gp0eulN8AAAAIdHJhbnNmZXIAAAADAAAAEgAAAAEFO1FR2Wg49QFY5KPOFAQ0bV5fN+7LD2GSQvOaHSH44QAAABIAAAAAAAAAAJWT/TG321X9xRsXneLBDe2QouWr3MbbBXmp32VGNmOVAAAACgAAAAAAAAAAAAAAADuaygAAAAABAAAAAQAAAAEFO1FR2Wg49QFY5KPOFAQ0bV5fN+7LD2GSQvOaHSH44QAAAY9SyLSVABbC/QAAABEAAAABAAAAAwAAAA8AAAASYXV0aGVudGljYXRvcl9kYXRhAAAAAAANAAAAJUmWDeWIDoxodDQXD2R2YFuP5K65ooYyx5lc87qDHZdjHQAAAAAAAAAAAAAPAAAAEGNsaWVudF9kYXRhX2pzb24AAAANAAAAcnsidHlwZSI6IndlYmF1dGhuLmdldCIsImNoYWxsZW5nZSI6ImhnMlRhOG8wWTliWFlyWlMyZjhzWk1kRFp6ektCSXhQNTZSd1FaNE90bTgiLCJvcmlnaW4iOiJodHRwOi8vbG9jYWxob3N0OjQ1MDcifQAAAAAADwAAAAlzaWduYXR1cmUAAAAAAAANAAAAQBcpuTFMxzkAdBs+5VIyJCBHaNuwEAva+kZVET4YuHVKF8gNII567RhxsnhBBSo5dDvssTN6vf2i42eEty66MtoAAAAAAAAAAX3B7N+TNRmfyZGNvgxzLOHRFGqo8pzJw2CvxqdHrpTfAAAACHRyYW5zZmVyAAAAAwAAABIAAAABBTtRUdloOPUBWOSjzhQENG1eXzfuyw9hkkLzmh0h+OEAAAASAAAAAAAAAACVk/0xt9tV/cUbF53iwQ3tkKLlq9zG2wV5qd9lRjZjlQAAAAoAAAAAAAAAAAAAAAA7msoAAAAAAAAAAAEAAAAAAAAAAwAAAAYAAAABfcHs35M1GZ/JkY2+DHMs4dEUaqjynMnDYK/Gp0eulN8AAAAUAAAAAQAAAAYAAAABBTtRUdloOPUBWOSjzhQENG1eXzfuyw9hkkLzmh0h+OEAAAAUAAAAAQAAAAeTiL4Gr2piUAmsXTev1ZzJ4kE2NUGZ0QMObd05iAMyzAAAAAMAAAAGAAAAAX3B7N+TNRmfyZGNvgxzLOHRFGqo8pzJw2CvxqdHrpTfAAAAEAAAAAEAAAACAAAADwAAAAdCYWxhbmNlAAAAABIAAAABBTtRUdloOPUBWOSjzhQENG1eXzfuyw9hkkLzmh0h+OEAAAABAAAAAAAAAACVk/0xt9tV/cUbF53iwQ3tkKLlq9zG2wV5qd9lRjZjlQAAAAYAAAABBTtRUdloOPUBWOSjzhQENG1eXzfuyw9hkkLzmh0h+OEAAAAVAAABj1LItJUAAAAAAEyTowAAGMgAAAG4AAAAAAADJBsAAAABRjZjlQAAAEASFnAIzNqpfdzv6yT0rSLMUDFgt7a/inCHurNCG55Jp8Imho04qRH+JNdkq0BgMC7yAJqH4N6Y2iGflFt3Lp4L";

    let expected_hash = "bcc9fa60c8f6607c981d6e1c65d77ae07617720113f9080fe5883d8e4a331a68";

    let hash = sandbox
        .new_assert_cmd("tx")
        .arg("hash")
        .write_stdin(xdr_base64.as_bytes())
        .assert()
        .success()
        .stdout_as_str();

    assert_eq!(hash.trim(), expected_hash);
}

#[tokio::test]
async fn build_simulate_sign_send() {
    let sandbox = &TestEnv::new();
    build_sim_sign_send(sandbox, "test", "--sign-with-key=test").await;
}

pub(crate) async fn build_sim_sign_send(sandbox: &TestEnv, account: &str, sign_with: &str) {
    sandbox
        .new_assert_cmd("contract")
        .arg("install")
        .args([
            "--wasm",
            HELLO_WORLD.path().as_os_str().to_str().unwrap(),
            "--source",
            account,
        ])
        .assert()
        .success();

    let tx_simulated = deploy_contract(
        sandbox,
        HELLO_WORLD,
        DeployOptions {
            kind: DeployKind::SimOnly,
            ..Default::default()
        },
    )
    .await;
    dbg!("{tx_simulated}");

    let tx_signed = sandbox
        .new_assert_cmd("tx")
        .arg("sign")
        .arg(sign_with)
        .write_stdin(tx_simulated.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    dbg!("{tx_signed}");

    sandbox
        .new_assert_cmd("tx")
        .arg("send")
        .write_stdin(tx_signed.as_bytes())
        .assert()
        .success()
        .stdout(predicates::str::contains("SUCCESS"));
}
