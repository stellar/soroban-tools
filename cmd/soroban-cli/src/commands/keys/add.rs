use clap::command;

use crate::{
    commands::global,
    config::{
        address::KeyName,
        key::{self, Key},
        locator,
        secret::{self, Secret},
    },
    print::Print,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Secret(#[from] secret::Error),
    #[error(transparent)]
    Key(#[from] key::Error),
    #[error(transparent)]
    Config(#[from] locator::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
pub struct Cmd {
    /// Name of identity
    pub name: KeyName,

    #[command(flatten)]
    pub secrets: secret::Args,

    #[command(flatten)]
    pub config_locator: locator::Args,

    /// Add a public key, ed25519, or muxed account, e.g. G1.., M2..
    #[arg(long, conflicts_with = "seed_phrase", conflicts_with = "secret_key")]
    pub public_key: Option<String>,
}

impl Cmd {
    pub fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let key = if let Some(key) = self.public_key.as_ref() {
            key.parse()?
        } else {
            self.secrets.read_secret()?.into()
        };

        let print = Print::new(global_args.quiet);
        let path = self.config_locator.write_key(&self.name, &key)?;

        if let Key::Secret(Secret::SeedPhrase { seed_phrase }) = key {
            if seed_phrase.split_whitespace().count() < 24 {
                print.warnln("The provided seed phrase lacks sufficient entropy and should be avoided. Using a 24-word seed phrase is a safer option.".to_string());
                print.warnln(
                    "To generate a new key, use the `stellar keys generate` command.".to_string(),
                );
            }
        }

        print.checkln(format!("Key saved with alias {:?} in {path:?}", self.name));

        Ok(())
    }
}
