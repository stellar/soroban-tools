#![allow(
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::missing_panics_doc
)]
use std::path::Path;

pub(crate) use soroban_rpc as rpc;
pub use stellar_xdr::curr as xdr;

mod cli;
pub use cli::main;

pub mod assembled;
pub mod commands;
pub mod config;
pub mod fee;
pub mod get_spec;
pub mod key;
pub mod log;
pub mod print;
pub mod signer;
pub mod toid;
pub mod tx;
pub mod upgrade_check;
pub mod utils;
pub mod wasm;

pub use commands::Root;

pub fn parse_cmd<T>(s: &str) -> Result<T, clap::Error>
where
    T: clap::CommandFactory + clap::FromArgMatches,
{
    let input = shlex::split(s).ok_or_else(|| {
        clap::Error::raw(
            clap::error::ErrorKind::InvalidValue,
            format!("Invalid input for command:\n{s}"),
        )
    })?;
    T::from_arg_matches_mut(&mut T::command().no_binary_name(true).get_matches_from(input))
}

pub trait CommandParser<T> {
    fn parse(s: &str) -> Result<T, clap::Error>;

    fn parse_arg_vec(s: &[&str]) -> Result<T, clap::Error>;
}

impl<T> CommandParser<T> for T
where
    T: clap::CommandFactory + clap::FromArgMatches,
{
    fn parse(s: &str) -> Result<T, clap::Error> {
        parse_cmd(s)
    }

    fn parse_arg_vec(args: &[&str]) -> Result<T, clap::Error> {
        T::from_arg_matches_mut(&mut T::command().no_binary_name(true).get_matches_from(args))
    }
}

pub trait Pwd {
    fn set_pwd(&mut self, pwd: &Path);
}

#[cfg(test)]
mod test {
    use std::path::{Path, PathBuf};

    #[test]
    #[ignore]
    fn md_gen() {
        doc_gen().unwrap();
    }

    fn doc_gen() -> std::io::Result<()> {
        let out_dir = project_root();
        let options = clap_markdown::MarkdownOptions::new()
            .show_footer(false)
            .show_table_of_contents(false)
            .title("Stellar CLI Manual".to_string());

        let content = clap_markdown::help_markdown_custom::<super::Root>(&options);

        std::fs::write(out_dir.join("FULL_HELP_DOCS.md"), content)?;

        Ok(())
    }

    fn project_root() -> PathBuf {
        Path::new(&env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .to_path_buf()
    }
}
