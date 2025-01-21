use std::str::FromStr;

use async_trait::async_trait;
use clap::{command, error::ErrorKind, CommandFactory, FromArgMatches, Parser};

use crate::config;

pub mod cache;
pub mod completion;
pub mod container;
pub mod contract;
pub mod env;
pub mod events;
pub mod global;
pub mod keys;
pub mod licenses;
pub mod network;
pub mod plugin;
pub mod snapshot;
pub mod tx;
pub mod version;

pub mod txn_result;

pub const HEADING_RPC: &str = "Options (RPC)";
pub const HEADING_GLOBAL: &str = "Options (Global)";
const ABOUT: &str =
    "Work seamlessly with Stellar accounts, contracts, and assets from the command line.

- Generate and manage keys and accounts
- Build, deploy, and interact with contracts
- Deploy asset contracts
- Stream events
- Start local testnets
- Decode, encode XDR
- More!

For additional information see:

- Stellar Docs: https://developers.stellar.org
- Smart Contract Docs: https://developers.stellar.org/docs/build/smart-contracts/overview
- CLI Docs: https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli";

// long_about is shown when someone uses `--help`; short help when using `-h`
const LONG_ABOUT: &str = "

To get started generate a new identity:

    stellar keys generate alice

Use keys with the `--source` flag in other commands.

Commands that work with contracts are organized under the `contract` subcommand. List them:

    stellar contract --help

Use contracts like a CLI:

    stellar contract invoke --id CCR6QKTWZQYW6YUJ7UP7XXZRLWQPFRV6SWBLQS4ZQOSAF4BOUD77OTE2 --source alice --network testnet -- --help

Anything after the `--` double dash (the \"slop\") is parsed as arguments to the contract-specific CLI, generated on-the-fly from the contract schema. For the hello world example, with a function called `hello` that takes one string argument `to`, here's how you invoke it:

    stellar contract invoke --id CCR6QKTWZQYW6YUJ7UP7XXZRLWQPFRV6SWBLQS4ZQOSAF4BOUD77OTE2 --source alice --network testnet -- hello --to world
";

#[derive(Parser, Debug)]
#[command(
    name = "stellar",
    about = ABOUT,
    version = version::long(),
    long_about = ABOUT.to_string() + LONG_ABOUT,
    disable_help_subcommand = true,
)]
pub struct Root {
    #[clap(flatten)]
    pub global_args: global::Args,

    #[command(subcommand)]
    pub cmd: Cmd,
}

impl Root {
    pub fn new() -> Result<Self, Error> {
        Self::try_parse().map_err(|e| {
            if std::env::args().any(|s| s == "--list") {
                let plugins = plugin::list().unwrap_or_default();
                if plugins.is_empty() {
                    println!("No Plugins installed. E.g. soroban-hello");
                } else {
                    println!("Installed Plugins:\n    {}", plugins.join("\n    "));
                }
                std::process::exit(0);
            }
            match e.kind() {
                ErrorKind::InvalidSubcommand => match plugin::run() {
                    Ok(()) => Error::Clap(e),
                    Err(e) => Error::Plugin(e),
                },
                _ => Error::Clap(e),
            }
        })
    }

    pub fn from_arg_matches<I, T>(itr: I) -> Result<Self, clap::Error>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        Self::from_arg_matches_mut(&mut Self::command().get_matches_from(itr))
    }
    pub async fn run(&mut self) -> Result<(), Error> {
        match &mut self.cmd {
            Cmd::Completion(completion) => completion.run(),
            Cmd::Contract(contract) => contract.run(&self.global_args).await?,
            Cmd::Events(events) => events.run().await?,
            Cmd::Xdr(xdr) => xdr.run()?,
            Cmd::Network(network) => network.run(&self.global_args).await?,
            Cmd::Container(container) => container.run(&self.global_args).await?,
            Cmd::Snapshot(snapshot) => snapshot.run(&self.global_args).await?,
            Cmd::Version(version) => version.run(),
            Cmd::Licenses(licenses) => licenses.run(),
            Cmd::Keys(id) => id.run(&self.global_args).await?,
            Cmd::Tx(tx) => tx.run(&self.global_args).await?,
            Cmd::Cache(cache) => cache.run()?,
            Cmd::Env(env) => env.run(&self.global_args)?,
        };
        Ok(())
    }
}

impl FromStr for Root {
    type Err = clap::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_arg_matches(s.split_whitespace())
    }
}

#[derive(Parser, Debug)]
pub enum Cmd {
    /// Tools for smart contract developers
    #[command(subcommand)]
    Contract(contract::Cmd),

    /// Watch the network for contract events
    Events(events::Cmd),

    /// Prints the environment variables
    ///
    /// Prints to stdout in a format that can be used as .env file. Environment
    /// variables have precedence over defaults.
    ///
    /// If there are no environment variables in use, prints the defaults.
    Env(env::Cmd),

    /// Create and manage identities including keys and addresses
    #[command(subcommand)]
    Keys(keys::Cmd),

    /// Configure connection to networks
    #[command(subcommand)]
    Network(network::Cmd),

    /// Start local networks in containers
    #[command(subcommand)]
    Container(container::Cmd),

    /// Download a snapshot of a ledger from an archive.
    #[command(subcommand)]
    Snapshot(snapshot::Cmd),

    /// Sign, Simulate, and Send transactions
    #[command(subcommand)]
    Tx(tx::Cmd),

    /// Decode and encode XDR
    Xdr(stellar_xdr::cli::Root),

    /// Print shell completion code for the specified shell.
    #[command(long_about = completion::LONG_ABOUT)]
    Completion(completion::Cmd),

    /// Cache for transactions and contract specs
    #[command(subcommand)]
    Cache(cache::Cmd),

    /// Print version information
    Version(version::Cmd),

    /// Show dependency licenses
    Licenses(licenses::Cmd),
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    // TODO: stop using Debug for displaying errors
    #[error(transparent)]
    Contract(#[from] contract::Error),

    #[error(transparent)]
    Events(#[from] events::Error),

    #[error(transparent)]
    Keys(#[from] keys::Error),

    #[error(transparent)]
    Xdr(#[from] stellar_xdr::cli::Error),

    #[error(transparent)]
    Clap(#[from] clap::error::Error),

    #[error(transparent)]
    Plugin(#[from] plugin::Error),

    #[error(transparent)]
    Network(#[from] network::Error),

    #[error(transparent)]
    Container(#[from] container::Error),

    #[error(transparent)]
    Snapshot(#[from] snapshot::Error),

    #[error(transparent)]
    Tx(#[from] tx::Error),

    #[error(transparent)]
    Cache(#[from] cache::Error),

    #[error(transparent)]
    Env(#[from] env::Error),
}

#[async_trait]
pub trait NetworkRunnable {
    type Error;
    type Result;

    async fn run_against_rpc_server(
        &self,
        global_args: Option<&global::Args>,
        config: Option<&config::Args>,
    ) -> Result<Self::Result, Self::Error>;
}
