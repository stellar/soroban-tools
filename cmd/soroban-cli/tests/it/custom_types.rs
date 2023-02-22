use serde_json::json;

use soroban_test::TestEnv;

use crate::util::{invoke, invoke_with_roundtrip};

#[test]
fn symbol() {
    invoke(&TestEnv::default(), "hello")
        .arg("--hello")
        .arg("world")
        .assert()
        .success()
        .stdout(
            r#""world"
"#,
        );
}

#[test]
fn symbol_with_quotes() {
    invoke_with_roundtrip("hello", json!("world"));
}

#[test]
fn generate_help() {
    invoke(&TestEnv::default(), "test")
        .arg("--help")
        .assert()
        .success();
}

#[test]
fn multi_arg() {
    invoke(&TestEnv::default(), "multi_args")
        .arg("--b")
        .assert()
        .success()
        .stderr("error: Missing argument a\n");
}

#[test]
fn multi_arg_success() {
    invoke(&TestEnv::default(), "multi_args")
        .arg("--a")
        .arg("42")
        .arg("--b")
        .assert()
        .success()
        .stdout("42\n");
}

#[test]
fn map() {
    invoke_with_roundtrip("map", json!({"0": true, "1": false}));
}

#[test]
fn map_help() {
    invoke(&TestEnv::default(), "map")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("Map<u32, bool>"));
}

#[test]
fn set() {
    invoke_with_roundtrip("set", json!([0, 1]));
}

#[test]
fn set_help() {
    invoke(&TestEnv::default(), "set")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("Set<u32>"));
}

#[test]
fn vec_() {
    invoke_with_roundtrip("vec", json!([0, 1]));
}

#[test]
fn vec_help() {
    invoke(&TestEnv::default(), "vec")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("Array<u32>"));
}

#[test]
fn tuple() {
    invoke_with_roundtrip("tuple", json!(["hello", 0]));
}

#[test]
fn tuple_help() {
    invoke(&TestEnv::default(), "tuple")
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("Tuple<Symbol, u32>"));
}

#[test]
fn strukt() {
    invoke_with_roundtrip("strukt", json!({"a": 42, "b": true, "c": "world"}));
}

#[test]
fn enum_2_str() {
    invoke_with_roundtrip("simple", json!("First"));
}

#[test]
fn e_2_s_enum() {
    invoke_with_roundtrip("complex", json!({"Enum": "First"}));
}

#[test]
fn e_2_s_tuple() {
    invoke_with_roundtrip(
        "complex",
        json!({"Tuple": [{"a": 42, "b": true, "c": "world"}, "First"]}),
    );
}

#[test]
fn e_2_s_strukt() {
    invoke_with_roundtrip(
        "complex",
        json!({"Struct": {"a": 42, "b": true, "c": "world"}}),
    );
}

#[test]
fn number_arg() {
    invoke_with_roundtrip("u32_", 42);
}

#[test]
fn i32() {
    invoke_with_roundtrip("i32_", 42);
}

#[test]
fn handle_arg_larger_than_i32() {
    invoke(&TestEnv::default(), "i32_")
        .arg("--i32_")
        .arg(u32::MAX.to_string())
        .assert()
        .success()
        .stderr(predicates::str::contains("value is not parseable"));
}

#[test]
fn handle_arg_larger_than_i64() {
    invoke(&TestEnv::default(), "i64_")
        .arg("--i64_")
        .arg(u64::MAX.to_string())
        .assert()
        .success()
        .stderr(predicates::str::contains("value is not parseable"));
}

#[test]
fn i64() {
    invoke_with_roundtrip("i64_", i64::MAX);
}

#[test]
fn negative_i32() {
    invoke_with_roundtrip("i32_", -42);
}

#[test]
fn negative_i64() {
    invoke_with_roundtrip("i64_", i64::MIN);
}

#[test]
fn account_address() {
    invoke_with_roundtrip(
        "address",
        json!("GD5KD2KEZJIGTC63IGW6UMUSMVUVG5IHG64HUTFWCHVZH2N2IBOQN7PS"),
    );
}

#[test]
fn contract_address() {
    invoke_with_roundtrip(
        "address",
        json!("CA3D5KRYM6CB7OWQ6TWYRR3Z4T7GNZLKERYNZGGA5SOAOPIFY6YQGAXE"),
    );
}

#[test]
fn bytes() {
    invoke_with_roundtrip("bytes", json!("7374656c6c6172"));
}

#[test]
fn const_enum() {
    invoke_with_roundtrip("card", "11");
}

#[test]
fn parse_u128() {
    let num = "340000000000000000000000000000000000000";
    invoke(&TestEnv::default(), "u128")
        .arg("--u128")
        .arg(num)
        .assert()
        .success()
        .stdout(format!(
            r#""{num}"
"#,
        ));
}

#[test]
fn parse_i128() {
    let num = "170000000000000000000000000000000000000";
    invoke(&TestEnv::default(), "i128")
        .arg("--i128")
        .arg(num)
        .assert()
        .success()
        .stdout(format!(
            r#""{num}"
"#,
        ));
}

#[test]
fn parse_negative_i128() {
    let num = "-170000000000000000000000000000000000000";
    invoke(&TestEnv::default(), "i128")
        .arg("--i128")
        .arg(num)
        .assert()
        .success()
        .stdout(format!(
            r#""{num}"
"#,
        ));
}

#[test]
fn boolean() {
    invoke(&TestEnv::default(), "boolean")
        .arg("--boolean")
        .assert()
        .success()
        .stdout(
            r#"true
"#,
        );
}
#[test]
fn boolean_no_flag() {
    invoke(&TestEnv::default(), "boolean")
        .assert()
        .success()
        .stdout(
            r#"false
"#,
        );
}

#[test]
fn boolean_not() {
    invoke(&TestEnv::default(), "not")
        .arg("--boolean")
        .assert()
        .success()
        .stdout(
            r#"false
"#,
        );
}

#[test]
fn boolean_not_no_flag() {
    invoke(&TestEnv::default(), "not")
        .assert()
        .success()
        .stdout(
            r#"true
"#,
        );
}
