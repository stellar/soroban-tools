Soroban Test
============

Test framework wrapping Soroban CLI.

Provides a way to run tests against a local sandbox; running against RPC endpoint _coming soon_.


Overview
========

- `TestEnv` is a test environment for running tests isolated from each other.
- `TestEnv::with_default` invokes a closure, which is passed a reference to a random `TestEnv`.
- `TestEnv::new_assert_cmd` creates an `assert_cmd::Command` for a given subcommand and sets the current
   directory to be the same as `TestEnv`.
- `TestEnv::cmd` is a generic function which parses a command from a string.
   Note, however, that it uses `shlex` to tokenize the string. This can cause issues
   for commands which contain strings with `"`s. For example, `{"hello": "world"}` becomes
   `{hello:world}`. For that reason it's recommended to use `TestEnv::cmd_arr` instead.
- `TestEnv::cmd_arr` is a generic function which takes an array of `&str` which is passed directly to clap.
   This is the preferred way since it ensures no string parsing footguns.
- `TestEnv::invoke` a convenience function for using the invoke command.


Example
=======

```rs
use soroban_test::{TestEnv, Wasm};

const WASM: &Wasm = &Wasm::Release("soroban_hello_world_contract");
const FRIEND: &str = "friend";

#[test]
fn invoke() {
    TestEnv::with_default(|workspace| {
        assert_eq!(
            format!("[\"Hello\",\"{FRIEND}\"]"),
            workspace
                .invoke(&[
                    "--id",
                    "1",
                    "--wasm",
                    &WASM.path().to_string_lossy(),
                    "--",
                    "hello",
                    "--to",
                    FRIEND,
                ])
                .unwrap()
        );
    });
}
```

Integration tests in Crate
==============

Currently all tests that require an RPC server are hidden behind a `it` feature, [found here](./tests/it/integration). To allow Rust-Analyzer to see the tests in vscode, `.vscode/settings.json`. Without RA, you can't follow through definitions and more importantly see errors before running tests.
