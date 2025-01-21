use serde_json::json;

use soroban_cli::commands;
use soroban_test::TestEnv;

use crate::integration::util::{deploy_custom, extend_contract};

use super::util::{invoke, invoke_with_roundtrip};

fn invoke_custom(e: &TestEnv, id: &str, func: &str) -> assert_cmd::Command {
    let mut s = e.new_assert_cmd("contract");
    s.arg("invoke").arg("--id").arg(id).arg("--").arg(func);
    s
}

#[tokio::test]
async fn parse() {
    let sandbox = &TestEnv::new();
    let id = &deploy_custom(sandbox).await;
    extend_contract(sandbox, id).await;
    symbol(sandbox, id);
    string_with_quotes(sandbox, id).await;
    symbol_with_quotes(sandbox, id).await;
    multi_arg_success(sandbox, id);
    bytes_as_file(sandbox, id);
    map(sandbox, id).await;
    vec_(sandbox, id).await;
    tuple(sandbox, id).await;
    strukt(sandbox, id).await;
    tuple_strukt(sandbox, id).await;
    enum_2_str(sandbox, id).await;
    e_2_s_enum(sandbox, id).await;
    asset(sandbox, id).await;
    e_2_s_tuple(sandbox, id).await;
    e_2_s_strukt(sandbox, id).await;
    number_arg(sandbox, id).await;
    number_arg_return_err(sandbox, id).await;
    i32(sandbox, id).await;
    i64(sandbox, id).await;
    negative_i32(sandbox, id).await;
    negative_i64(sandbox, id).await;
    account_address(sandbox, id).await;
    account_address_with_alias(sandbox, id).await;
    contract_address(sandbox, id).await;
    contract_address_with_alias(sandbox, id).await;
    bytes(sandbox, id).await;
    const_enum(sandbox, id).await;
    number_arg_return_ok(sandbox, id);
    void(sandbox, id);
    val(sandbox, id);
    parse_u128(sandbox, id);
    parse_i128(sandbox, id);
    parse_negative_i128(sandbox, id);
    parse_u256(sandbox, id);
    parse_i256(sandbox, id);
    parse_negative_i256(sandbox, id);
    boolean(sandbox, id);
    boolean_two(sandbox, id);
    boolean_no_flag(sandbox, id);
    boolean_false(sandbox, id);
    boolean_not(sandbox, id);
    boolean_not_no_flag(sandbox, id);
    option_none(sandbox, id);
    option_some(sandbox, id);
}

fn symbol(sandbox: &TestEnv, id: &str) {
    invoke_custom(sandbox, id, "hello")
        .arg("--hello")
        .arg("world")
        .assert()
        .success()
        .stdout(
            r#""world"
"#,
        );
}

async fn string_with_quotes(sandbox: &TestEnv, id: &str) {
    invoke_with_roundtrip(sandbox, id, "string", json!("hello world")).await;
}

async fn symbol_with_quotes(sandbox: &TestEnv, id: &str) {
    invoke_with_roundtrip(sandbox, id, "hello", json!("world")).await;
}

fn multi_arg_success(sandbox: &TestEnv, id: &str) {
    invoke_custom(sandbox, id, "multi_args")
        .arg("--a")
        .arg("42")
        .arg("--b")
        .assert()
        .success()
        .stdout("42\n");
}

fn bytes_as_file(sandbox: &TestEnv, id: &str) {
    let env = &TestEnv::default();
    let path = env.temp_dir.join("bytes.txt");
    std::fs::write(&path, 0x0073_7465_6c6c_6172u128.to_be_bytes()).unwrap();
    invoke_custom(sandbox, id, "bytes")
        .arg("--bytes-file-path")
        .arg(path)
        .assert()
        .success()
        .stdout("\"0000000000000000007374656c6c6172\"\n");
}

async fn map(sandbox: &TestEnv, id: &str) {
    invoke_with_roundtrip(sandbox, id, "map", json!({"0": true, "1": false})).await;
}

async fn vec_(sandbox: &TestEnv, id: &str) {
    invoke_with_roundtrip(sandbox, id, "vec", json!([0, 1])).await;
}

async fn tuple(sandbox: &TestEnv, id: &str) {
    invoke_with_roundtrip(sandbox, id, "tuple", json!(["hello", 0])).await;
}

async fn strukt(sandbox: &TestEnv, id: &str) {
    invoke_with_roundtrip(
        sandbox,
        id,
        "strukt",
        json!({"a": 42, "b": true, "c": "world"}),
    )
    .await;
}

async fn tuple_strukt(sandbox: &TestEnv, id: &str) {
    invoke_with_roundtrip(
        sandbox,
        id,
        "tuple_strukt",
        json!([{"a": 42, "b": true, "c": "world"}, "First"]),
    )
    .await;
}

async fn enum_2_str(sandbox: &TestEnv, id: &str) {
    invoke_with_roundtrip(sandbox, id, "simple", json!("First")).await;
}

async fn e_2_s_enum(sandbox: &TestEnv, id: &str) {
    invoke_with_roundtrip(sandbox, id, "complex", json!({"Enum": "First"})).await;
}

async fn asset(sandbox: &TestEnv, id: &str) {
    invoke_with_roundtrip(
        sandbox,
        id,
        "complex",
        json!({"Asset": ["CB64D3G7SM2RTH6JSGG34DDTFTQ5CFDKVDZJZSODMCX4NJ2HV2KN7OHT", "100" ]}),
    )
    .await;
}

fn complex_tuple() -> serde_json::Value {
    json!({"Tuple": [{"a": 42, "b": true, "c": "world"}, "First"]})
}

async fn e_2_s_tuple(sandbox: &TestEnv, id: &str) {
    invoke_with_roundtrip(sandbox, id, "complex", complex_tuple()).await;
}

async fn e_2_s_strukt(sandbox: &TestEnv, id: &str) {
    invoke_with_roundtrip(
        sandbox,
        id,
        "complex",
        json!({"Struct": {"a": 42, "b": true, "c": "world"}}),
    )
    .await;
}

async fn number_arg(sandbox: &TestEnv, id: &str) {
    invoke_with_roundtrip(sandbox, id, "u32_", 42).await;
}

fn number_arg_return_ok(sandbox: &TestEnv, id: &str) {
    invoke_custom(sandbox, id, "u32_fail_on_even")
        .arg("--u32_")
        .arg("1")
        .assert()
        .success()
        .stdout("1\n");
}

async fn number_arg_return_err(sandbox: &TestEnv, id: &str) {
    let res = sandbox
        .invoke_with_test(&["--id", id, "--", "u32_fail_on_even", "--u32_=2"])
        .await
        .unwrap_err();
    if let commands::contract::invoke::Error::ContractInvoke(name, doc) = &res {
        assert_eq!(name, "NumberMustBeOdd");
        assert_eq!(doc, "Please provide an odd number");
    };
    println!("{res:#?}");
}

fn void(sandbox: &TestEnv, id: &str) {
    invoke_custom(sandbox, id, "woid")
        .assert()
        .success()
        .stdout("\n");
}

fn val(sandbox: &TestEnv, id: &str) {
    invoke_custom(sandbox, id, "val")
        .assert()
        .success()
        .stdout("null\n");
}

async fn i32(sandbox: &TestEnv, id: &str) {
    invoke_with_roundtrip(sandbox, id, "i32_", 42).await;
}

async fn i64(sandbox: &TestEnv, id: &str) {
    invoke_with_roundtrip(sandbox, id, "i64_", i64::MAX).await;
}

async fn negative_i32(sandbox: &TestEnv, id: &str) {
    invoke_with_roundtrip(sandbox, id, "i32_", -42).await;
}

async fn negative_i64(sandbox: &TestEnv, id: &str) {
    invoke_with_roundtrip(sandbox, id, "i64_", i64::MIN).await;
}

async fn account_address(sandbox: &TestEnv, id: &str) {
    invoke_with_roundtrip(
        sandbox,
        id,
        "addresse",
        json!("GD5KD2KEZJIGTC63IGW6UMUSMVUVG5IHG64HUTFWCHVZH2N2IBOQN7PS"),
    )
    .await;
}

async fn account_address_with_alias(sandbox: &TestEnv, id: &str) {
    let res = invoke(sandbox, id, "addresse", &json!("test").to_string()).await;
    let test = format!("\"{}\"", super::tx::operations::test_address(sandbox));
    assert_eq!(test, res);
}

async fn contract_address(sandbox: &TestEnv, id: &str) {
    invoke_with_roundtrip(
        sandbox,
        id,
        "addresse",
        json!("CA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAXE"),
    )
    .await;
}

async fn contract_address_with_alias(sandbox: &TestEnv, id: &str) {
    sandbox
        .new_assert_cmd("contract")
        .arg("alias")
        .arg("add")
        .arg("test_contract")
        .arg("--id=CA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAXE")
        .assert()
        .success();
    let res = invoke(sandbox, id, "addresse", &json!("test_contract").to_string()).await;
    assert_eq!(
        res,
        "\"CA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAXE\""
    );
}

async fn bytes(sandbox: &TestEnv, id: &str) {
    invoke_with_roundtrip(sandbox, id, "bytes", json!("7374656c6c6172")).await;
}

async fn const_enum(sandbox: &TestEnv, id: &str) {
    invoke_with_roundtrip(sandbox, id, "card", "11").await;
}

fn parse_u128(sandbox: &TestEnv, id: &str) {
    let num = "340000000000000000000000000000000000000";
    invoke_custom(sandbox, id, "u128")
        .arg("--u128")
        .arg(num)
        .assert()
        .success()
        .stdout(format!(
            r#""{num}"
"#,
        ));
}

fn parse_i128(sandbox: &TestEnv, id: &str) {
    let num = "170000000000000000000000000000000000000";
    invoke_custom(sandbox, id, "i128")
        .arg("--i128")
        .arg(num)
        .assert()
        .success()
        .stdout(format!(
            r#""{num}"
"#,
        ));
}

fn parse_negative_i128(sandbox: &TestEnv, id: &str) {
    let num = "-170000000000000000000000000000000000000";
    invoke_custom(sandbox, id, "i128")
        .arg("--i128")
        .arg(num)
        .assert()
        .success()
        .stdout(format!(
            r#""{num}"
"#,
        ));
}

fn parse_u256(sandbox: &TestEnv, id: &str) {
    let num = "340000000000000000000000000000000000000";
    invoke_custom(sandbox, id, "u256")
        .arg("--u256")
        .arg(num)
        .assert()
        .success()
        .stdout(format!(
            r#""{num}"
"#,
        ));
}

fn parse_i256(sandbox: &TestEnv, id: &str) {
    let num = "170000000000000000000000000000000000000";
    invoke_custom(sandbox, id, "i256")
        .arg("--i256")
        .arg(num)
        .assert()
        .success()
        .stdout(format!(
            r#""{num}"
"#,
        ));
}

fn parse_negative_i256(sandbox: &TestEnv, id: &str) {
    let num = "-170000000000000000000000000000000000000";
    invoke_custom(sandbox, id, "i256")
        .arg("--i256")
        .arg(num)
        .assert()
        .success()
        .stdout(format!(
            r#""{num}"
"#,
        ));
}

fn boolean(sandbox: &TestEnv, id: &str) {
    invoke_custom(sandbox, id, "boolean")
        .arg("--boolean")
        .assert()
        .success()
        .stdout(
            r"true
",
        );
}
fn boolean_two(sandbox: &TestEnv, id: &str) {
    invoke_custom(sandbox, id, "boolean")
        .arg("--boolean")
        .arg("true")
        .assert()
        .success()
        .stdout(
            r"true
",
        );
}

fn boolean_no_flag(sandbox: &TestEnv, id: &str) {
    invoke_custom(sandbox, id, "boolean")
        .assert()
        .success()
        .stdout(
            r"false
",
        );
}

fn boolean_false(sandbox: &TestEnv, id: &str) {
    invoke_custom(sandbox, id, "boolean")
        .arg("--boolean")
        .arg("false")
        .assert()
        .success()
        .stdout(
            r"false
",
        );
}

fn boolean_not(sandbox: &TestEnv, id: &str) {
    invoke_custom(sandbox, id, "not")
        .arg("--boolean")
        .assert()
        .success()
        .stdout(
            r"false
",
        );
}

fn boolean_not_no_flag(sandbox: &TestEnv, id: &str) {
    invoke_custom(sandbox, id, "not").assert().success().stdout(
        r"true
",
    );
}

fn option_none(sandbox: &TestEnv, id: &str) {
    invoke_custom(sandbox, id, "option")
        .assert()
        .success()
        .stdout(
            r"null
",
        );
}

fn option_some(sandbox: &TestEnv, id: &str) {
    invoke_custom(sandbox, id, "option")
        .arg("--option=1")
        .assert()
        .success()
        .stdout(
            r"1
",
        );
}
