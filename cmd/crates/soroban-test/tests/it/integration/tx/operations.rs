use soroban_cli::{
    config::locator,
    tx::{builder, ONE_XLM},
    utils::contract_id_hash_from_asset,
    xdr::{self, ReadXdr, SequenceNumber},
};
use soroban_rpc::LedgerEntryResult;
use soroban_test::{AssertExt, TestEnv};

use crate::integration::{
    hello_world::invoke_hello_world,
    util::{deploy_contract, DeployOptions, HELLO_WORLD},
};

pub fn test_address(sandbox: &TestEnv) -> String {
    sandbox
        .new_assert_cmd("keys")
        .arg("address")
        .arg("test")
        .assert()
        .success()
        .stdout_as_str()
}

fn new_account(sandbox: &TestEnv, name: &str) -> String {
    sandbox.generate_account(name, None).assert().success();
    sandbox
        .new_assert_cmd("keys")
        .args(["address", name])
        .assert()
        .success()
        .stdout_as_str()
}

fn gen_account_no_fund(sandbox: &TestEnv, name: &str) -> String {
    sandbox
        .new_assert_cmd("keys")
        .args(["generate", "--no-fund", name])
        .assert()
        .success();
    sandbox
        .new_assert_cmd("keys")
        .args(["address", name])
        .assert()
        .success()
        .stdout_as_str()
}

// returns test and test1 addresses
fn setup_accounts(sandbox: &TestEnv) -> (String, String) {
    (test_address(sandbox), new_account(sandbox, "test1"))
}

#[tokio::test]
async fn create_account() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("keys")
        .args(["generate", "--no-fund", "new"])
        .assert()
        .success();

    let address = sandbox
        .new_assert_cmd("keys")
        .args(["address", "new"])
        .assert()
        .success()
        .stdout_as_str();
    let test = test_address(sandbox);
    let client = sandbox.network.rpc_client().unwrap();
    let test_account = client.get_account(&test).await.unwrap();
    println!("test account has a balance of {}", test_account.balance);
    let starting_balance = ONE_XLM * 100;
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "create-account",
            "--destination",
            address.as_str(),
            "--starting-balance",
            starting_balance.to_string().as_str(),
        ])
        .assert()
        .success();
    let test_account_after = client.get_account(&test).await.unwrap();
    assert!(test_account_after.balance < test_account.balance);
    let id = deploy_contract(
        sandbox,
        HELLO_WORLD,
        DeployOptions {
            deployer: Some("new".to_string()),
            ..Default::default()
        },
    )
    .await;
    println!("{id}");
    invoke_hello_world(sandbox, &id);
}

#[tokio::test]
async fn create_account_with_alias() {
    let sandbox = &TestEnv::new();
    sandbox
        .new_assert_cmd("keys")
        .args(["generate", "--no-fund", "new"])
        .assert()
        .success();
    let test = test_address(sandbox);
    let client = sandbox.client();
    let test_account = client.get_account(&test).await.unwrap();
    println!("test account has a balance of {}", test_account.balance);
    let starting_balance = ONE_XLM * 100;
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "create-account",
            "--destination",
            "new",
            "--starting-balance",
            starting_balance.to_string().as_str(),
        ])
        .assert()
        .success();
    let test_account_after = client.get_account(&test).await.unwrap();
    assert!(test_account_after.balance < test_account.balance);
    let id = deploy_contract(
        sandbox,
        HELLO_WORLD,
        DeployOptions {
            deployer: Some("new".to_string()),
            ..Default::default()
        },
    )
    .await;
    println!("{id}");
    invoke_hello_world(sandbox, &id);
}

#[tokio::test]
async fn payment_with_alias() {
    let sandbox = &TestEnv::new();
    let client = sandbox.client();
    let (test, test1) = setup_accounts(sandbox);
    let test_account = client.get_account(&test).await.unwrap();
    println!("test account has a balance of {}", test_account.balance);

    let before = client.get_account(&test).await.unwrap();
    let test1_account_entry_before = client.get_account(&test1).await.unwrap();

    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "payment",
            "--destination",
            "test1",
            "--amount",
            ONE_XLM.to_string().as_str(),
        ])
        .assert()
        .success();
    let test1_account_entry = client.get_account(&test1).await.unwrap();
    assert_eq!(
        ONE_XLM,
        test1_account_entry.balance - test1_account_entry_before.balance,
        "Should have One XLM more"
    );
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(before.balance - 10_000_100, after.balance);
}

#[tokio::test]
async fn payment() {
    let sandbox = &TestEnv::new();
    let client = sandbox.network.rpc_client().unwrap();
    let (test, test1) = setup_accounts(sandbox);
    let test_account = client.get_account(&test).await.unwrap();
    println!("test account has a balance of {}", test_account.balance);

    let before = client.get_account(&test).await.unwrap();
    let test1_account_entry_before = client.get_account(&test1).await.unwrap();

    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "payment",
            "--destination",
            test1.as_str(),
            "--amount",
            "10_000_000",
        ])
        .assert()
        .success();
    let test1_account_entry = client.get_account(&test1).await.unwrap();
    assert_eq!(
        ONE_XLM,
        test1_account_entry.balance - test1_account_entry_before.balance,
        "Should have One XLM more"
    );
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(before.balance - 10_000_100, after.balance);
}

#[tokio::test]
async fn bump_sequence() {
    let sandbox = &TestEnv::new();
    let client = sandbox.network.rpc_client().unwrap();
    let test = test_address(sandbox);
    let before = client.get_account(&test).await.unwrap();
    let amount = 50;
    let seq = SequenceNumber(before.seq_num.0 + amount);
    // bump sequence tx new
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "bump-sequence",
            "--bump-to",
            seq.0.to_string().as_str(),
        ])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(seq, after.seq_num);
}

#[tokio::test]
async fn account_merge() {
    let sandbox = &TestEnv::new();
    let client = sandbox.network.rpc_client().unwrap();
    let (test, test1) = setup_accounts(sandbox);
    let before = client.get_account(&test).await.unwrap();
    let before1 = client.get_account(&test1).await.unwrap();
    let fee = 100;
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "account-merge",
            "--source",
            "test1",
            "--account",
            test.as_str(),
            "--fee",
            fee.to_string().as_str(),
        ])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert!(client.get_account(&test1).await.is_err());
    assert_eq!(before.balance + before1.balance - fee, after.balance);
}

#[tokio::test]
async fn account_merge_with_alias() {
    let sandbox = &TestEnv::new();
    let client = sandbox.client();
    let (test, test1) = setup_accounts(sandbox);
    let before = client.get_account(&test).await.unwrap();
    let before1 = client.get_account(&test1).await.unwrap();
    let fee = 100;
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "account-merge",
            "--source",
            "test1",
            "--account",
            "test",
            "--fee",
            fee.to_string().as_str(),
        ])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert!(client.get_account(&test1).await.is_err());
    assert_eq!(before.balance + before1.balance - fee, after.balance);
}

#[tokio::test]
async fn set_trustline_flags() {
    let sandbox = &TestEnv::new();
    let (test, test1_address) = setup_accounts(sandbox);
    let asset = "usdc:test1";
    issue_asset(sandbox, &test, asset, 100_000, 100).await;
    sandbox
        .new_assert_cmd("contract")
        .arg("asset")
        .arg("deploy")
        .arg("--asset")
        .arg(asset)
        .assert()
        .success();
    let id = contract_id_hash_from_asset(
        &format!("usdc:{test1_address}")
            .parse::<builder::Asset>()
            .unwrap()
            .resolve(&locator::Args::default())
            .unwrap(),
        &sandbox.network.network_passphrase,
    );
    // sandbox
    //     .new_assert_cmd("contract")
    //     .args([
    //         "invoke",
    //         "--id",
    //         &id.to_string(),
    //         "--",
    //         "authorized",
    //         "--id",
    //         &test,
    //     ])
    //     .assert()
    //     .success()
    //     .stdout("false\n");

    sandbox
        .new_assert_cmd("contract")
        .args([
            "invoke",
            "--id",
            &id.to_string(),
            "--",
            "authorized",
            "--id",
            &test,
        ])
        .assert()
        .success()
        .stdout("true\n");
}

#[tokio::test]
async fn set_options_add_signer() {
    let sandbox = &TestEnv::new();
    let client = sandbox.network.rpc_client().unwrap();
    let (test, test1) = setup_accounts(sandbox);
    let before = client.get_account(&test).await.unwrap();
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-options",
            "--signer",
            test1.as_str(),
            "--signer-weight",
            "1",
        ])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(before.signers.len() + 1, after.signers.len());
    assert_eq!(after.signers.first().unwrap().key, test1.parse().unwrap());
    let key = xdr::LedgerKey::Account(xdr::LedgerKeyAccount {
        account_id: test.parse().unwrap(),
    });
    let res = client.get_ledger_entries(&[key]).await.unwrap();
    let xdr_str = res.entries.unwrap().clone().first().unwrap().clone().xdr;
    let entry = xdr::LedgerEntryData::from_xdr_base64(&xdr_str, xdr::Limits::none()).unwrap();
    let xdr::LedgerEntryData::Account(xdr::AccountEntry { signers, .. }) = entry else {
        panic!();
    };
    assert_eq!(signers.first().unwrap().key, test1.parse().unwrap());

    // Now remove signer with a weight of 0
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-options",
            "--signer",
            test1.as_str(),
            "--signer-weight",
            "0",
        ])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(before.signers.len(), after.signers.len());
}

fn build_and_run(sandbox: &TestEnv, cmd: &str, args: &[&str]) -> String {
    let mut args_2 = args.to_vec();
    args_2.push("--build-only");
    let res = sandbox
        .new_assert_cmd(cmd)
        .args(args_2)
        .assert()
        .success()
        .stdout_as_str();
    sandbox.new_assert_cmd(cmd).args(args).assert().success();
    res
}

#[tokio::test]
async fn set_options() {
    let sandbox = &TestEnv::new();
    let client = sandbox.network.rpc_client().unwrap();
    let (test, alice) = setup_accounts(sandbox);
    let before = client.get_account(&test).await.unwrap();
    assert!(before.inflation_dest.is_none());
    let tx_xdr = build_and_run(
        sandbox,
        "tx",
        &[
            "new",
            "set-options",
            "--inflation-dest",
            alice.as_str(),
            "--home-domain",
            "test.com",
            "--master-weight=100",
            "--med-threshold=100",
            "--low-threshold=100",
            "--high-threshold=100",
            "--signer",
            alice.as_str(),
            "--signer-weight=100",
            "--set-required",
            "--set-revocable",
            "--set-clawback-enabled",
            "--set-immutable",
        ],
    );
    println!("{tx_xdr}");
    let after = client.get_account(&test).await.unwrap();
    println!("{before:#?}\n{after:#?}");
    assert_eq!(
        after.flags,
        xdr::AccountFlags::ClawbackEnabledFlag as u32
            | xdr::AccountFlags::ImmutableFlag as u32
            | xdr::AccountFlags::RevocableFlag as u32
            | xdr::AccountFlags::RequiredFlag as u32
    );
    assert_eq!([100, 100, 100, 100], after.thresholds.0);
    assert_eq!(100, after.signers[0].weight);
    assert_eq!(alice, after.signers[0].key.to_string());
    let xdr::PublicKey::PublicKeyTypeEd25519(xdr::Uint256(key)) = after.inflation_dest.unwrap().0;
    assert_eq!(alice, stellar_strkey::ed25519::PublicKey(key).to_string());
    assert_eq!("test.com", after.home_domain.to_string());
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-options",
            "--inflation-dest",
            test.as_str(),
            "--home-domain",
            "test.com",
            "--master-weight=100",
            "--med-threshold=100",
            "--low-threshold=100",
            "--high-threshold=100",
            "--signer",
            alice.as_str(),
            "--signer-weight=100",
            "--set-required",
            "--set-revocable",
            "--set-clawback-enabled",
        ])
        .assert()
        .failure();
}

#[tokio::test]
async fn set_some_options() {
    let sandbox = &TestEnv::new();
    let client = sandbox.network.rpc_client().unwrap();
    let test = test_address(sandbox);
    let before = client.get_account(&test).await.unwrap();
    assert!(before.inflation_dest.is_none());
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-options",
            "--set-clawback-enabled",
            "--master-weight=100",
        ])
        .assert()
        .failure()
        .stderr(predicates::str::contains("AuthRevocableRequired"));
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-options",
            "--set-revocable",
            "--master-weight=100",
        ])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(after.flags, xdr::AccountFlags::RevocableFlag as u32);
    assert_eq!([100, 0, 0, 0], after.thresholds.0);
    assert!(after.inflation_dest.is_none());
    assert_eq!(
        after.home_domain,
        "".parse::<xdr::StringM<32>>().unwrap().into()
    );
    assert!(after.signers.is_empty());
    sandbox
        .new_assert_cmd("tx")
        .args(["new", "set-options", "--set-clawback-enabled"])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(
        after.flags,
        xdr::AccountFlags::RevocableFlag as u32 | xdr::AccountFlags::ClawbackEnabledFlag as u32
    );
    sandbox
        .new_assert_cmd("tx")
        .args(["new", "set-options", "--clear-clawback-enabled"])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(after.flags, xdr::AccountFlags::RevocableFlag as u32);
    sandbox
        .new_assert_cmd("tx")
        .args(["new", "set-options", "--clear-revocable"])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(after.flags, 0);
    sandbox
        .new_assert_cmd("tx")
        .args(["new", "set-options", "--set-required"])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(after.flags, xdr::AccountFlags::RequiredFlag as u32);
    sandbox
        .new_assert_cmd("tx")
        .args(["new", "set-options", "--clear-required"])
        .assert()
        .success();
    let after = client.get_account(&test).await.unwrap();
    assert_eq!(after.flags, 0);
}

#[tokio::test]
async fn change_trust() {
    let sandbox = &TestEnv::new();
    let (test, issuer) = setup_accounts(sandbox);
    let asset = &format!("usdc:{issuer}");

    let limit = 100_000_000;
    let half_limit = limit / 2;
    issue_asset(sandbox, &test, asset, limit, half_limit).await;
    sandbox
        .new_assert_cmd("contract")
        .arg("asset")
        .arg("deploy")
        .arg("--asset")
        .arg(asset)
        .assert()
        .success();

    // wrap_cmd(&asset).run().await.unwrap();
    let id = contract_id_hash_from_asset(
        &asset
            .parse::<builder::Asset>()
            .unwrap()
            .resolve(&locator::Args::default())
            .unwrap(),
        &sandbox.network.network_passphrase,
    );
    sandbox
        .new_assert_cmd("contract")
        .args([
            "invoke",
            "--id",
            &id.to_string(),
            "--",
            "balance",
            "--id",
            &test,
        ])
        .assert()
        .stdout(format!("\"{half_limit}\"\n"));

    let bob = new_account(sandbox, "bob");
    let bobs_limit = half_limit / 2;
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "change-trust",
            "--source=bob",
            "--line",
            asset,
            "--limit",
            bobs_limit.to_string().as_str(),
        ])
        .assert()
        .success();
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "payment",
            "--destination",
            &bob,
            "--asset",
            asset,
            "--amount",
            half_limit.to_string().as_str(),
        ])
        .assert()
        .failure();
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "payment",
            "--destination",
            &bob,
            "--asset",
            asset,
            "--amount",
            bobs_limit.to_string().as_str(),
        ])
        .assert()
        .success();
    sandbox
        .new_assert_cmd("contract")
        .args([
            "invoke",
            "--id",
            &id.to_string(),
            "--",
            "balance",
            "--id",
            &bob,
        ])
        .assert()
        .stdout(format!("\"{bobs_limit}\"\n"));
}

#[tokio::test]
async fn manage_data() {
    let sandbox = &TestEnv::new();
    let (test, _) = setup_accounts(sandbox);
    let client = sandbox.network.rpc_client().unwrap();
    let key = "test";
    let value = "beefface";
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "manage-data",
            "--data-name",
            key,
            "--data-value",
            value,
        ])
        .assert()
        .success();
    let account_id = xdr::AccountId(xdr::PublicKey::PublicKeyTypeEd25519(xdr::Uint256(
        stellar_strkey::ed25519::PublicKey::from_string(&test)
            .unwrap()
            .0,
    )));
    let orig_data_name: xdr::StringM<64> = key.parse().unwrap();
    let res = client
        .get_ledger_entries(&[xdr::LedgerKey::Data(xdr::LedgerKeyData {
            account_id,
            data_name: orig_data_name.clone().into(),
        })])
        .await
        .unwrap();
    let value_res = res.entries.as_ref().unwrap().first().unwrap();
    let ledeger_entry_data =
        xdr::LedgerEntryData::from_xdr_base64(&value_res.xdr, xdr::Limits::none()).unwrap();
    let xdr::LedgerEntryData::Data(xdr::DataEntry {
        data_value,
        data_name,
        ..
    }) = ledeger_entry_data
    else {
        panic!("Expected DataEntry");
    };
    assert_eq!(data_name, orig_data_name.into());
    assert_eq!(hex::encode(data_value.0.to_vec()), value);
}

async fn issue_asset(sandbox: &TestEnv, test: &str, asset: &str, limit: u64, initial_balance: u64) {
    let client = sandbox.network.rpc_client().unwrap();
    let test_before = client.get_account(test).await.unwrap();
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "change-trust",
            "--line",
            asset,
            "--limit",
            limit.to_string().as_str(),
        ])
        .assert()
        .success();

    sandbox
        .new_assert_cmd("tx")
        .args(["new", "set-options", "--set-required"])
        .assert()
        .success();
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "set-trustline-flags",
            "--asset",
            asset,
            "--trustor",
            test,
            "--set-authorize",
            "--source",
            "test1",
        ])
        .assert()
        .success();

    let after = client.get_account(test).await.unwrap();
    assert_eq!(test_before.num_sub_entries + 1, after.num_sub_entries);
    println!("aa");
    // Send a payment to the issuer
    sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "payment",
            "--destination",
            test,
            "--asset",
            asset,
            "--amount",
            initial_balance.to_string().as_str(),
            "--source=test1",
        ])
        .assert()
        .success();
}

#[tokio::test]
async fn multi_create_accounts() {
    let sandbox = &TestEnv::new();
    let client = sandbox.network.rpc_client().unwrap();
    let nums: Vec<u8> = (1..=3).collect();
    let mut accounts: Vec<(String, String)> = nums
        .iter()
        .map(|x| {
            let name = format!("test_{x}");
            let address = gen_account_no_fund(sandbox, &name);
            (name, address)
        })
        .collect();
    let (_, test_99_address) = accounts.pop().unwrap();

    let input = sandbox
        .new_assert_cmd("tx")
        .args([
            "new",
            "create-account",
            "--fee=1000000",
            "--build-only",
            "--destination",
            &test_99_address,
        ])
        .assert()
        .success()
        .stdout_as_str();

    let final_tx = accounts.iter().fold(input, |tx_env, (_, address)| {
        sandbox
            .new_assert_cmd("tx")
            .args(["op", "add", "create-account", "--destination", address])
            .write_stdin(tx_env.as_bytes())
            .assert()
            .success()
            .stdout_as_str()
    });
    let out = sandbox
        .new_assert_cmd("tx")
        .arg("send")
        .write_stdin(
            sandbox
                .new_assert_cmd("tx")
                .arg("sign")
                .arg("--sign-with-key=test")
                .write_stdin(final_tx.as_bytes())
                .assert()
                .success()
                .stdout_as_str()
                .as_bytes(),
        )
        .assert()
        .success()
        .stdout_as_str();
    println!("{out}");
    let keys = accounts
        .iter()
        .map(|(_, address)| {
            xdr::LedgerKey::Account(xdr::LedgerKeyAccount {
                account_id: address.parse().unwrap(),
            })
        })
        .collect::<Vec<_>>();

    let account = client.get_account(&test_99_address).await.unwrap();
    println!("{account:#?}");
    let entries = client.get_ledger_entries(&keys).await.unwrap();
    println!("{entries:#?}");
    entries
        .entries
        .unwrap()
        .iter()
        .for_each(|LedgerEntryResult { xdr, .. }| {
            let xdr::LedgerEntryData::Account(value) =
                xdr::LedgerEntryData::from_xdr_base64(xdr, xdr::Limits::none()).unwrap()
            else {
                panic!("Expected Account");
            };
            assert_eq!(value.balance, 10_000_000);
        });
}
