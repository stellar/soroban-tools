use soroban_cli::{
    commands::contract,
    config::{locator::KeyType, secret::Secret},
};
use soroban_test::{TestEnv, Wasm, TEST_ACCOUNT};
use std::path::Path;

pub const CUSTOM_TYPES: &Wasm = &Wasm::Custom("test-wasms", "test_custom_types");

#[derive(Clone)]
pub enum SecretKind {
    Seed,
    Key,
}

#[allow(clippy::needless_pass_by_value)]
pub fn add_key(dir: &Path, name: &str, kind: SecretKind, data: &str) {
    let secret = match kind {
        SecretKind::Seed => Secret::SeedPhrase {
            seed_phrase: data.to_string(),
        },
        SecretKind::Key => Secret::SecretKey {
            secret_key: data.to_string(),
        },
    };

    KeyType::Identity
        .write(name, &secret, &dir.join(".soroban"))
        .unwrap();
}

pub fn add_test_id(dir: &Path) -> String {
    let name = "test_id";
    add_key(
        dir,
        name,
        SecretKind::Key,
        "SBGWSG6BTNCKCOB3DIFBGCVMUPQFYPA2G4O34RMTB343OYPXU5DJDVMN",
    );
    name.to_owned()
}

pub const DEFAULT_SEED_PHRASE: &str =
    "coral light army gather adapt blossom school alcohol coral light army giggle";

pub async fn invoke_custom(
    sandbox: &TestEnv,
    id: &str,
    func: &str,
    arg: &str,
    wasm: &Path,
) -> Result<String, contract::invoke::Error> {
    let mut i: contract::invoke::Cmd =
        sandbox.cmd_with_config(&["--id", id, "--", func, arg], None);
    i.wasm = Some(wasm.to_path_buf());
    let s = sandbox.run_cmd_with(i, TEST_ACCOUNT).await;
    s.map(|tx| tx.into_result().unwrap())
}

pub const DEFAULT_CONTRACT_ID: &str = "CDR6QKTWZQYW6YUJ7UP7XXZRLWQPFRV6SWBLQS4ZQOSAF4BOUD77OO5Z";
#[allow(dead_code)]
pub const LOCAL_NETWORK_PASSPHRASE: &str = "Local Sandbox Stellar Network ; September 2022";
