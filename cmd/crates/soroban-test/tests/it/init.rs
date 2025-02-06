use assert_fs::prelude::*;
use predicates::prelude::predicate;
use soroban_test::TestEnv;

#[test]
fn init() {
    let sandbox = TestEnv::default();
    let cli_version = soroban_cli::commands::version::pkg();
    let major = cli_version.split('.').next().unwrap();
    let is_rc = cli_version.contains("rc");
    sandbox
        .new_assert_cmd("contract")
        .arg("init")
        .arg(".")
        .assert()
        .success();
    sandbox
        .dir()
        .child("Cargo.toml")
        .assert(predicate::function(|c: &str| {
            let table = toml::from_str::<toml::Table>(c).unwrap();
            let sdk_version = table["workspace"]["dependencies"]["soroban-sdk"].as_str();
            println!("Check expected version {major}.0.0 matches template's {sdk_version:?}");
            if is_rc {
                sdk_version.and_then(|x| x.split('-').next()) == Some(&format!("{major}.0.0"))
            } else {
                sdk_version == Some(&format!("{major}.0.0"))
            }
        }));
}
