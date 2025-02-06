use soroban_cli::commands::contract::{self, arg_parsing};
use soroban_test::TestEnv;

use crate::util::{invoke_custom as invoke, CUSTOM_TYPES, DEFAULT_CONTRACT_ID};

async fn invoke_custom(func: &str, args: &str) -> Result<String, contract::invoke::Error> {
    let e = &TestEnv::default();
    invoke(e, DEFAULT_CONTRACT_ID, func, args, &CUSTOM_TYPES.path()).await
}

#[tokio::test]
async fn generate_help() {
    assert!(invoke_custom("strukt_hel", "--help")
        .await
        .unwrap()
        .contains("Example contract method which takes a struct"));
}

#[tokio::test]
async fn vec_help() {
    assert!(invoke_custom("vec", "--help")
        .await
        .unwrap()
        .contains("Array<u32>"));
}

#[tokio::test]
async fn tuple_help() {
    assert!(invoke_custom("tuple", "--help")
        .await
        .unwrap()
        .contains("Tuple<Symbol, u32>"));
}

#[tokio::test]
async fn strukt_help() {
    let output = invoke_custom("strukt", "--help").await.unwrap();
    assert!(output.contains("--strukt '{ \"a\": 1, \"b\": true, \"c\": \"hello\" }'",));
    assert!(output.contains("This is from the rust doc above the struct Test",));
}

#[tokio::test]
async fn complex_enum_help() {
    let output = invoke_custom("complex", "--help").await.unwrap();
    assert!(output.contains(r#"--complex '{"Struct":{ "a": 1, "b": true, "c": "hello" }}"#,));
    assert!(output.contains(r#"{"Tuple":[{ "a": 1, "b": true, "c": "hello" }"#,));
    assert!(output.contains(r#"{"Enum":"First"|"Second"|"Third"}"#,));
    assert!(output.contains(
        r#"{"Asset":["GDIY6AQQ75WMD4W46EYB7O6UYMHOCGQHLAQGQTKHDX4J2DYQCHVCR4W4", "-100"]}"#,
    ));
    assert!(output.contains(r#""Void"'"#));
}

#[tokio::test]
async fn recursive_enum_help() {
    let output = invoke_custom("recursive_enum", "--help").await.unwrap();
    assert!(output.contains(r#"--complex"#,));
    assert!(output.contains(r#""Void"'"#));
}

#[tokio::test]
async fn multi_arg_failure() {
    assert!(matches!(
        invoke_custom("multi_args", "--b").await.unwrap_err(),
        contract::invoke::Error::ArgParsing(arg_parsing::Error::MissingArgument(_))
    ));
}

#[tokio::test]
async fn handle_arg_larger_than_i32_failure() {
    let res = invoke_custom("i32_", &format!("--i32_={}", u32::MAX)).await;
    assert!(matches!(
        res,
        Err(contract::invoke::Error::ArgParsing(
            arg_parsing::Error::CannotParseArg { .. }
        ))
    ));
}

#[tokio::test]
async fn handle_arg_larger_than_i64_failure() {
    let res = invoke_custom("i64_", &format!("--i64_={}", u64::MAX)).await;
    assert!(matches!(
        res,
        Err(contract::invoke::Error::ArgParsing(
            arg_parsing::Error::CannotParseArg { .. }
        ))
    ));
}

#[test]
fn build() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let hello_world_contract_path =
        cargo_dir.join("tests/fixtures/test-wasms/hello_world/Cargo.toml");
    sandbox
        .new_assert_cmd("contract")
        .arg("build")
        .arg("--manifest-path")
        .arg(hello_world_contract_path)
        .arg("--profile")
        .arg("test-wasms")
        .arg("--package")
        .arg("test_hello_world")
        .assert()
        .success();
}
