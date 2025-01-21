use soroban_cli::{
    commands,
    xdr::{Limits, WriteXdr},
};
use soroban_test::{TestEnv, Wasm};
use std::fmt::Display;

pub const HELLO_WORLD: &Wasm = &Wasm::Custom("test-wasms", "test_hello_world");
pub const CONSTRUCTOR: &Wasm = &Wasm::Custom("test-wasms", "test_constructor");
pub const CUSTOM_TYPES: &Wasm = &Wasm::Custom("test-wasms", "test_custom_types");
pub const CUSTOM_ACCOUNT: &Wasm = &Wasm::Custom("test-wasms", "test_custom_account");
pub const SWAP: &Wasm = &Wasm::Custom("test-wasms", "test_swap");

pub async fn invoke(sandbox: &TestEnv, id: &str, func: &str, data: &str) -> String {
    sandbox
        .invoke_with_test(&["--id", id, "--", func, &format!("--{func}"), data])
        .await
        .unwrap()
}
pub async fn invoke_with_roundtrip<D>(e: &TestEnv, id: &str, func: &str, data: D)
where
    D: Display,
{
    let data = data.to_string();
    println!("{data}");
    let res = invoke(e, id, func, &data).await;
    assert_eq!(res, data);
}

pub const TEST_SALT: &str = "f55ff16f66f43360266b95db6f8fec01d76031054306ae4a4b380598f6cfd114";

pub enum DeployKind {
    BuildOnly,
    Normal,
    SimOnly,
}

impl Display for DeployKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeployKind::BuildOnly => write!(f, "--build-only"),
            DeployKind::Normal => write!(f, ""),
            DeployKind::SimOnly => write!(f, "--sim-only"),
        }
    }
}

pub async fn deploy_hello(sandbox: &TestEnv) -> String {
    deploy_contract(sandbox, HELLO_WORLD, DeployKind::Normal, None).await
}

pub async fn deploy_custom(sandbox: &TestEnv) -> String {
    deploy_contract(sandbox, CUSTOM_TYPES, DeployKind::Normal, None).await
}

pub async fn deploy_swap(sandbox: &TestEnv) -> String {
    deploy_contract(sandbox, SWAP, DeployKind::Normal, None).await
}

pub async fn deploy_custom_account(sandbox: &TestEnv) -> String {
    deploy_contract(sandbox, CUSTOM_ACCOUNT, DeployKind::Normal, None).await
}

pub async fn deploy_contract(
    sandbox: &TestEnv,
    wasm: &Wasm<'static>,
    deploy: DeployKind,
    deployer: Option<&str>,
) -> String {
    let cmd = sandbox.cmd_with_config::<_, commands::contract::deploy::wasm::Cmd>(
        &[
            "--fee",
            "1000000",
            "--wasm",
            &wasm.path().to_string_lossy(),
            "--salt",
            TEST_SALT,
            "--ignore-checks",
            &deploy.to_string(),
        ],
        None,
    );
    let res = sandbox
        .run_cmd_with(cmd, deployer.unwrap_or("test"))
        .await
        .unwrap();
    match deploy {
        DeployKind::BuildOnly | DeployKind::SimOnly => match res.to_envelope() {
            commands::txn_result::TxnEnvelopeResult::TxnEnvelope(e) => {
                return e.to_xdr_base64(Limits::none()).unwrap()
            }
            commands::txn_result::TxnEnvelopeResult::Res(_) => todo!(),
        },
        DeployKind::Normal => (),
    }
    res.into_result().unwrap().to_string()
}

pub async fn extend_contract(sandbox: &TestEnv, id: &str) {
    extend(sandbox, id, None).await;
}

pub async fn extend(sandbox: &TestEnv, id: &str, value: Option<&str>) {
    let mut args = vec!["--id", id, "--ledgers-to-extend", "100001"];
    if let Some(value) = value {
        args.push("--key");
        args.push(value);
    }
    sandbox
        .new_assert_cmd("contract")
        .arg("extend")
        .args(args)
        .assert()
        .success();
}
