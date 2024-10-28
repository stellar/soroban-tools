use predicates::prelude::predicate;
use soroban_test::TestEnv;

#[test]
fn build_all() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace/");
    sandbox
        .new_assert_cmd("contract")
        .current_dir(fixture_path)
        .arg("build")
        .arg("--print-commands-only")
        .assert()
        .success()
        .stdout(predicate::eq("\
cargo rustc --manifest-path=contracts/add/Cargo.toml --crate-type=cdylib --target=wasm32-unknown-unknown --release
cargo rustc --manifest-path=contracts/call/Cargo.toml --crate-type=cdylib --target=wasm32-unknown-unknown --release
cargo rustc --manifest-path=contracts/add/add2/Cargo.toml --crate-type=cdylib --target=wasm32-unknown-unknown --release
"));
}

#[test]
fn build_package_by_name() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace/");
    sandbox
        .new_assert_cmd("contract")
        .current_dir(fixture_path)
        .arg("build")
        .arg("--print-commands-only")
        .arg("--package=add")
        .assert()
        .success()
        .stdout(predicate::eq("\
cargo rustc --manifest-path=contracts/add/Cargo.toml --crate-type=cdylib --target=wasm32-unknown-unknown --release
"));
}

#[test]
fn build_package_by_current_dir() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace/contracts/add");
    sandbox
        .new_assert_cmd("contract")
        .current_dir(fixture_path)
        .arg("build")
        .arg("--print-commands-only")
        .assert()
        .success()
        .stdout(predicate::eq(
            "\
cargo rustc --manifest-path=Cargo.toml --crate-type=cdylib --target=wasm32-unknown-unknown --release
",
        ));
}

#[test]
fn build_no_package_found() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace/");
    sandbox
        .new_assert_cmd("contract")
        .current_dir(fixture_path)
        .arg("build")
        .arg("--print-commands-only")
        .arg("--package=nopkgwiththisname")
        .assert()
        .failure()
        .stderr(predicate::eq(
            "\
❌ error: package nopkgwiththisname not found
",
        ));
}

#[test]
fn build_all_when_in_non_package_directory() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace/contracts/add/src/");
    sandbox
        .new_assert_cmd("contract")
        .current_dir(fixture_path)
        .arg("build")
        .arg("--print-commands-only")
        .assert()
        .success()
        .stdout(predicate::eq(
            "\
cargo rustc --manifest-path=../Cargo.toml --crate-type=cdylib --target=wasm32-unknown-unknown --release
cargo rustc --manifest-path=../../call/Cargo.toml --crate-type=cdylib --target=wasm32-unknown-unknown --release
cargo rustc --manifest-path=../add2/Cargo.toml --crate-type=cdylib --target=wasm32-unknown-unknown --release
",
        ));
}

#[test]
fn build_default_members() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace-with-default-members/");
    sandbox
        .new_assert_cmd("contract")
        .current_dir(fixture_path)
        .arg("build")
        .arg("--print-commands-only")
        .assert()
        .success()
        .stdout(predicate::eq(
            "\
cargo rustc --manifest-path=contracts/add/Cargo.toml --crate-type=cdylib --target=wasm32-unknown-unknown --release
",
        ));
}

#[test]
fn build_with_metadata() {
    let sandbox = TestEnv::default();
    let cargo_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_path = cargo_dir.join("tests/fixtures/workspace/contracts/add");
    let outdir = sandbox.dir().join("out");

    sandbox
        .new_assert_cmd("contract")
        .current_dir(&fixture_path)
        .arg("build")
        .arg("--meta")
        .arg("contract meta=added on build")
        .arg("--out-dir")
        .arg(&outdir)
        .assert()
        .success();

    // verify that the metadata added in the contract code via contractmetadata! macro is present
    // as well as the meta that is included on build
    let wasm_path = fixture_path.join(&outdir).join("add.wasm");
    sandbox
        .new_assert_cmd("contract")
        .current_dir(&fixture_path)
        .arg("info")
        .arg("meta")
        .arg("--wasm")
        .arg(wasm_path)
        .assert()
        .success()
        .stdout(predicate::str::contains("Description: A test add contract"))
        .stdout(predicate::str::contains("contract meta: added on build"));
}
