use soroban_test::{AssertExt, TestEnv};

#[test]
#[ignore]
fn init_and_deploy() {
    let name = "hello_world";
    let sandbox = TestEnv::default();

    sandbox
        .new_assert_cmd("contract")
        .arg("init")
        .arg("--name")
        .arg(name)
        .arg("project")
        .assert()
        .success();

    let manifest_path = sandbox
        .dir()
        .join(format!("project/contracts/{name}/Cargo.toml"));
    assert!(manifest_path.exists());

    sandbox
        .new_assert_cmd("contract")
        .arg("build")
        .arg("--manifest-path")
        .arg(manifest_path)
        .assert()
        .success();

    let target_dir = sandbox
        .dir()
        .join("project/target/wasm32-unknown-unknown/release");
    assert!(target_dir.exists());

    let assert = sandbox
        .new_assert_cmd("contract")
        .arg("deploy")
        .arg("--wasm")
        .arg(target_dir.join(format!("{name}.wasm")))
        .assert();

    let contract = assert.stdout_as_str();

    assert.success();

    let assert = sandbox
        .new_assert_cmd("contract")
        .arg("invoke")
        .arg("--id")
        .arg(contract)
        .arg("--")
        .arg("hello")
        .arg("--to")
        .arg("bar")
        .assert();

    let output = assert.stdout_as_str();

    assert_eq!(output, r#"["Hello","bar"]"#);

    assert.success();
}
