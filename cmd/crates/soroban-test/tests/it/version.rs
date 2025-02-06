use soroban_cli::commands::version::long;
use soroban_test::TestEnv;

#[test]
fn version() {
    let sandbox = TestEnv::default();
    sandbox
        .new_assert_cmd("version")
        .assert()
        .success()
        .stdout(format!("stellar {}\n", long()));
}
