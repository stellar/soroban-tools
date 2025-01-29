use soroban_cli::{config::locator, tx::builder, utils::contract_id_hash_from_asset};
use soroban_test::{AssertExt, TestEnv, LOCAL_NETWORK_PASSPHRASE};

#[tokio::test]
#[ignore]
async fn burn() {
    let sandbox = &TestEnv::new();
    let network_passphrase = LOCAL_NETWORK_PASSPHRASE.to_string();
    let address = sandbox
        .new_assert_cmd("keys")
        .arg("address")
        .arg("test")
        .assert()
        .stdout_as_str();
    let asset = "native";
    sandbox
        .new_assert_cmd("contract")
        .arg("asset")
        .arg("deploy")
        .arg("--source=test")
        .arg("--asset")
        .arg(asset)
        .assert()
        .success();
    let asset = asset
        .parse::<builder::Asset>()
        .unwrap()
        .resolve(&locator::Args::default())
        .unwrap();
    let hash = contract_id_hash_from_asset(&asset, &network_passphrase);
    let id = stellar_strkey::Contract(hash.0).to_string();
    sandbox
        .new_assert_cmd("contract")
        .args([
            "invoke",
            "--id",
            &id,
            "--",
            "balance",
            "--id",
            &address.to_string(),
        ])
        .assert()
        .stdout("\"9223372036854775807\"\n");
    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .args([
            "--id",
            &id,
            "--",
            "authorized",
            "--id",
            &address.to_string(),
        ])
        .assert()
        .stdout("true\n");
    sandbox
        .new_assert_cmd("contract")
        .args([
            "invoke",
            "--id",
            &id,
            "--",
            "balance",
            "--id",
            &address.to_string(),
        ])
        .assert()
        .stdout("\"9223372036854775807\"\n");
    sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .env("RUST_LOGS", "trace")
        .args([
            "--source=test",
            "--id",
            &id,
            "--",
            "burn",
            "--from",
            "test",
            "--amount=100",
        ])
        .assert()
        .stdout("")
        .stderr("");

    println!("hi");
    sandbox
        .new_assert_cmd("contract")
        .args([
            "invoke",
            "--id",
            &id,
            "--",
            "balance",
            "--id",
            &address.to_string(),
        ])
        .assert()
        .stdout("\"9223372036854775707\"\n");
}
