//! **Soroban Test** - Test framework for invoking Soroban externally.
//!
//! Currently soroban provides a mock test environment for writing unit tets.
//!
//! However, it does not provide a way to run tests against a local sandbox or rpc endpoint.
//!
//! ## Overview
//!
//! - `TestEnv` is a test environment for running tests isolated from each other.
//! - `TestEnv::with_default` invokes a closure, which is passed a reference to a random `TestEnv`.
//! - `TestEnv::new_assert_cmd` creates an `assert_cmd::Command` for a given subcommand and sets the current
//!    directory to be the same as `TestEnv`.
//! - `TestEnv::cmd` is a generic function which parses a command from a string.
//!    Note, however, that it uses `shlex` to tokenize the string. This can cause issues
//!    for commands which contain strings with `"`s. For example, `{"hello": "world"}` becomes
//!    `{hello:world}`. For that reason it's recommended to use `TestEnv::cmd_arr` instead.
//! - `TestEnv::cmd_arr` is a generic function which takes an array of `&str` which is passed directly to clap.
//!    This is the preferred way since it ensures no string parsing footguns.
//! - `TestEnv::invoke` a convenience function for using the invoke command.
//!
#![allow(
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::missing_panics_doc
)]
use std::{ffi::OsString, fmt::Display, path::Path};

use assert_cmd::{assert::Assert, Command};
use assert_fs::{fixture::FixtureError, prelude::PathChild, TempDir};
use fs_extra::dir::CopyOptions;

use soroban_cli::{
    commands::{contract::invoke, global, keys, NetworkRunnable},
    config::{self, network},
    CommandParser,
};

mod wasm;

pub use wasm::Wasm;

pub const TEST_ACCOUNT: &str = "test";

pub const LOCAL_NETWORK_PASSPHRASE: &str = "Standalone Network ; February 2017";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    TempDir(#[from] FixtureError),

    #[error(transparent)]
    FsError(#[from] fs_extra::error::Error),

    #[error(transparent)]
    Invoke(#[from] invoke::Error),
}

/// A `TestEnv` is a contained process for a specific test, with its own ENV and
/// its own `TempDir` where it will save test-specific configuration.
pub struct TestEnv {
    pub temp_dir: TempDir,
    pub network: network::Network,
}

impl Default for TestEnv {
    fn default() -> Self {
        let temp_dir = TempDir::new().unwrap();
        Self {
            temp_dir,
            network: network::Network {
                rpc_url: "http://localhost:8889/soroban/rpc".to_string(),
                network_passphrase: LOCAL_NETWORK_PASSPHRASE.to_string(),
                rpc_headers: [].to_vec(),
            },
        }
    }
}

impl TestEnv {
    /// Execute a closure which is passed a reference to the `TestEnv`.
    /// `TempDir` implements the `Drop` trait ensuring that the temporary directory
    /// it creates is deleted when the `TestEnv` is dropped. This pattern ensures
    /// that the `TestEnv` cannot be dropped by the closure. For this reason, it's
    /// recommended to use `TempDir::with_default` instead of `new` or `default`.
    ///
    /// ```rust,no_run
    /// use soroban_test::TestEnv;
    /// TestEnv::with_default(|env| {
    ///     env.new_assert_cmd("contract").args(&["invoke", "--id", "1", "--", "hello", "--world=world"]).assert().success();
    /// });
    /// ```
    ///
    pub fn with_default<F: FnOnce(&TestEnv)>(f: F) {
        let test_env = TestEnv::default();
        f(&test_env);
    }

    pub fn with_default_network<F: FnOnce(&TestEnv)>(f: F) {
        let test_env = TestEnv::new();
        f(&test_env);
    }

    pub fn with_port(host_port: u16) -> TestEnv {
        Self::with_rpc_url(&format!("http://localhost:{host_port}/soroban/rpc"))
    }

    pub fn with_rpc_url(rpc_url: &str) -> TestEnv {
        let mut env = TestEnv::default();
        env.network.rpc_url = rpc_url.to_string();
        if let Ok(network_passphrase) = std::env::var("STELLAR_NETWORK_PASSPHRASE") {
            env.network.network_passphrase = network_passphrase;
        };
        env.generate_account("test", None).assert().success();
        env
    }

    pub fn with_rpc_provider(rpc_url: &str, rpc_headers: Vec<(String, String)>) -> TestEnv {
        let mut env = TestEnv::default();
        env.network.rpc_url = rpc_url.to_string();
        env.network.rpc_headers = rpc_headers;
        if let Ok(network_passphrase) = std::env::var("STELLAR_NETWORK_PASSPHRASE") {
            env.network.network_passphrase = network_passphrase;
        };
        env.generate_account("test", None).assert().success();
        env
    }

    pub fn new() -> TestEnv {
        if let Ok(rpc_url) = std::env::var("SOROBAN_RPC_URL") {
            return Self::with_rpc_url(&rpc_url);
        }
        if let Ok(rpc_url) = std::env::var("STELLAR_RPC_URL") {
            return Self::with_rpc_url(&rpc_url);
        }
        let host_port = std::env::var("SOROBAN_PORT")
            .as_deref()
            .ok()
            .and_then(|n| n.parse().ok())
            .unwrap_or(8000);
        Self::with_port(host_port)
    }
    /// Create a new `assert_cmd::Command` for a given subcommand and set's the current directory
    /// to be the internal `temp_dir`.
    pub fn new_assert_cmd(&self, subcommand: &str) -> Command {
        let mut cmd: Command = self.bin();

        cmd.arg(subcommand)
            .env("SOROBAN_ACCOUNT", TEST_ACCOUNT)
            .env("SOROBAN_RPC_URL", &self.network.rpc_url)
            .env("SOROBAN_NETWORK_PASSPHRASE", LOCAL_NETWORK_PASSPHRASE)
            .env("XDG_CONFIG_HOME", self.temp_dir.join("config").as_os_str())
            .env("XDG_DATA_HOME", self.temp_dir.join("data").as_os_str())
            .current_dir(&self.temp_dir);

        if !self.network.rpc_headers.is_empty() {
            cmd.env(
                "STELLAR_RPC_HEADERS",
                format!(
                    "{}:{}",
                    &self.network.rpc_headers[0].0, &self.network.rpc_headers[0].1
                ),
            );
        }

        cmd
    }

    pub fn bin(&self) -> Command {
        Command::cargo_bin("soroban").unwrap_or_else(|_| Command::new("soroban"))
    }

    pub fn generate_account(&self, account: &str, seed: Option<String>) -> Command {
        let mut cmd = self.new_assert_cmd("keys");
        cmd.arg("generate").arg(account);
        if let Some(seed) = seed {
            cmd.arg(format!("--seed={seed}"));
        }
        cmd
    }

    pub fn fund_account(&self, account: &str) -> Assert {
        self.new_assert_cmd("keys")
            .arg("fund")
            .arg(account)
            .assert()
    }

    /// Parses a `&str` into a command and sets the pwd to be the same as the current `TestEnv`.
    /// Uses shlex under the hood and thus has issues parsing strings with embedded `"`s.
    /// Thus `TestEnv::cmd_arr` is recommended to instead.
    pub fn cmd<T: CommandParser<T>>(&self, args: &str) -> T {
        Self::cmd_with_pwd(args, self.dir())
    }

    /// Same as `TestEnv::cmd` but sets the pwd can be used instead of the current `TestEnv`.
    pub fn cmd_with_pwd<T: CommandParser<T>>(args: &str, pwd: &Path) -> T {
        let args = format!("--config-dir={pwd:?} {args}");
        T::parse(&args).unwrap()
    }

    /// Same as `TestEnv::cmd_arr` but sets the pwd can be used instead of the current `TestEnv`.
    pub fn cmd_arr_with_pwd<T: CommandParser<T>>(args: &[&str], pwd: &Path) -> T {
        let mut cmds = vec!["--config-dir", pwd.to_str().unwrap()];
        cmds.extend_from_slice(args);
        T::parse_arg_vec(&cmds).unwrap()
    }

    /// Parse a command using an array of `&str`s, which passes the strings directly to clap
    /// avoiding some issues `cmd` has with shlex. Use the current `TestEnv` pwd.
    pub fn cmd_arr<T: CommandParser<T>>(&self, args: &[&str]) -> T {
        Self::cmd_arr_with_pwd(args, self.dir())
    }

    /// A convenience method for using the invoke command.
    pub async fn invoke_with_test<I: AsRef<str>>(
        &self,
        command_str: &[I],
    ) -> Result<String, invoke::Error> {
        self.invoke_with(command_str, "test").await
    }

    /// A convenience method for using the invoke command.
    pub async fn invoke_with<I: AsRef<str>>(
        &self,
        command_str: &[I],
        source: &str,
    ) -> Result<String, invoke::Error> {
        let cmd = self.cmd_with_config::<I, invoke::Cmd>(command_str, None);
        self.run_cmd_with(cmd, source)
            .await
            .map(|r| r.into_result().unwrap())
    }

    /// A convenience method for using the invoke command.
    pub fn cmd_with_config<I: AsRef<str>, T: CommandParser<T> + NetworkRunnable>(
        &self,
        command_str: &[I],
        source_account: Option<&str>,
    ) -> T {
        let source = source_account.unwrap_or("test");
        let source_str = format!("--source-account={source}");
        let mut arg = vec![
            "--network=local",
            "--rpc-url=http",
            "--network-passphrase=AA",
            source_str.as_str(),
        ];
        let input = command_str
            .iter()
            .map(AsRef::as_ref)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>();
        arg.extend(input);
        T::parse_arg_vec(&arg).unwrap()
    }

    pub fn clone_config(&self, account: &str) -> config::Args {
        let config_dir = Some(self.dir().to_path_buf());
        config::Args {
            network: network::Args {
                rpc_url: Some(self.network.rpc_url.clone()),
                rpc_headers: [].to_vec(),
                network_passphrase: Some(LOCAL_NETWORK_PASSPHRASE.to_string()),
                network: None,
            },
            source_account: account.parse().unwrap(),
            locator: config::locator::Args {
                global: false,
                config_dir,
            },
            hd_path: None,
        }
    }

    /// Invoke an already parsed invoke command
    pub async fn run_cmd_with<T: NetworkRunnable>(
        &self,
        cmd: T,
        account: &str,
    ) -> Result<T::Result, T::Error> {
        let config = self.clone_config(account);
        cmd.run_against_rpc_server(
            Some(&global::Args {
                locator: config.locator.clone(),
                filter_logs: Vec::default(),
                quiet: false,
                verbose: false,
                very_verbose: false,
                list: false,
                no_cache: false,
            }),
            Some(&config),
        )
        .await
    }

    /// Reference to current directory of the `TestEnv`.
    pub fn dir(&self) -> &TempDir {
        &self.temp_dir
    }

    /// Returns the public key corresponding to the test keys's `hd_path`
    pub async fn test_address(&self, hd_path: usize) -> String {
        self.cmd::<keys::public_key::Cmd>(&format!("--hd-path={hd_path}"))
            .public_key()
            .await
            .unwrap()
            .to_string()
    }

    /// Returns the private key corresponding to the test keys's `hd_path`
    pub fn test_show(&self, hd_path: usize) -> String {
        self.cmd::<keys::secret::Cmd>(&format!("--hd-path={hd_path}"))
            .private_key()
            .unwrap()
            .to_string()
    }

    /// Copy the contents of the current `TestEnv` to another `TestEnv`
    pub fn fork(&self) -> Result<TestEnv, Error> {
        let this = TestEnv::new();
        self.save(&this.temp_dir)?;
        Ok(this)
    }

    /// Save the current state of the `TestEnv` to the given directory.
    pub fn save(&self, dst: &Path) -> Result<(), Error> {
        fs_extra::dir::copy(&self.temp_dir, dst, &CopyOptions::new())?;
        Ok(())
    }

    pub fn client(&self) -> soroban_rpc::Client {
        self.network.rpc_client().unwrap()
    }

    #[cfg(feature = "emulator-tests")]
    pub async fn speculos_container(
        ledger_device_model: &str,
    ) -> testcontainers::ContainerAsync<stellar_ledger::emulator_test_support::speculos::Speculos>
    {
        use stellar_ledger::emulator_test_support::{
            enable_hash_signing, get_container, wait_for_emulator_start_text,
        };
        let container = get_container(ledger_device_model).await;
        let ui_host_port: u16 = container.get_host_port_ipv4(5000).await.unwrap();
        wait_for_emulator_start_text(ui_host_port).await;
        enable_hash_signing(ui_host_port).await;
        container
    }
}

pub fn temp_ledger_file() -> OsString {
    TempDir::new()
        .unwrap()
        .child("ledger.json")
        .as_os_str()
        .into()
}

pub trait AssertExt {
    fn stdout_as_str(&self) -> String;
    fn stderr_as_str(&self) -> String;
}

impl AssertExt for Assert {
    fn stdout_as_str(&self) -> String {
        String::from_utf8(self.get_output().stdout.clone())
            .expect("failed to make str")
            .trim()
            .to_owned()
    }
    fn stderr_as_str(&self) -> String {
        String::from_utf8(self.get_output().stderr.clone())
            .expect("failed to make str")
            .trim()
            .to_owned()
    }
}
pub trait CommandExt {
    fn json_arg<A>(&mut self, j: A) -> &mut Self
    where
        A: Display;
}

impl CommandExt for Command {
    fn json_arg<A>(&mut self, j: A) -> &mut Self
    where
        A: Display,
    {
        self.arg(OsString::from(j.to_string()))
    }
}
