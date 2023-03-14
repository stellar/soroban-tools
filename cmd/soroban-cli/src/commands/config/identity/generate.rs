use super::super::{
    locator,
    secret::{self, Secret},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),
    #[error(transparent)]
    Secret(#[from] secret::Error),
}

#[derive(Debug, clap::Args)]
pub struct Cmd {
    /// Name of identity
    pub name: String,

    /// Optional seed to use when generating seed phrase.
    /// Random otherwise.
    #[clap(long, conflicts_with = "default-seed")]
    pub seed: Option<String>,

    /// Output the generated identity as a secret key
    #[clap(long, short = 's')]
    pub as_secret: bool,

    #[clap(flatten)]
    pub config_locator: locator::Args,

    /// When generating a secret key, which hd_path should be used from the original seed_phrase.
    #[clap(long)]
    pub hd_path: Option<usize>,

    /// Generate the default seed phrase. Useful for testing.
    /// Equivalent to --seed 0000000000000000
    #[clap(long, short = 'd', conflicts_with = "seed")]
    pub default_seed: bool,
}

impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let seed_phrase = if self.default_seed {
            Secret::test_seed_phrase()
        } else {
            Secret::from_seed(self.seed.as_deref())
        }?;
        let secret = if self.as_secret {
            let secret = seed_phrase.private_key(self.hd_path)?;
            Secret::SecretKey {
                secret_key: secret.to_string(),
            }
        } else {
            seed_phrase
        };
        self.config_locator.write_identity(&self.name, &secret)?;
        Ok(())
    }
}
