use soroban_sdk::xdr::{Limits, ReadXdr, TransactionEnvelope, WriteXdr};
use soroban_test::{AssertExt, TestEnv};

use crate::integration::util::{deploy_contract, DeployKind, HELLO_WORLD};

#[tokio::test]
async fn txn_simulate() {
    let sandbox = &TestEnv::new();
    let xdr_base64_build_only = deploy_contract(sandbox, HELLO_WORLD, DeployKind::BuildOnly).await;
    let xdr_base64_sim_only = deploy_contract(sandbox, HELLO_WORLD, DeployKind::SimOnly).await;
    let tx_env =
        TransactionEnvelope::from_xdr_base64(&xdr_base64_build_only, Limits::none()).unwrap();
    let tx = soroban_cli::commands::tx::xdr::unwrap_envelope_v1(tx_env).unwrap();
    let assembled_str = sandbox
        .new_assert_cmd("tx")
        .arg("simulate")
        .write_stdin(xdr_base64_build_only.as_bytes())
        .assert()
        .success()
        .stdout_as_str();
    assert_eq!(xdr_base64_sim_only, assembled_str);
    let assembled = sandbox
        .client()
        .simulate_and_prepare_transaction(&tx)
        .await
        .unwrap();
    let txn_env: TransactionEnvelope = assembled
        .assembled
        .expect("Simulation should succeed")
        .transaction()
        .clone()
        .into();
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
