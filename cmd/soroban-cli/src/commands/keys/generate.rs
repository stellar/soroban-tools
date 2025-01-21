use clap::{arg, command};
use sep5::SeedPhrase;

use super::super::config::{
    locator, network,
    secret::{self, Secret},
};
use crate::{
    commands::global,
    config::address::KeyName,
    print::Print,
    signer::keyring::{self, StellarEntry},
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Config(#[from] locator::Error),

    #[error(transparent)]
    Secret(#[from] secret::Error),

    #[error(transparent)]
    Network(#[from] network::Error),

    #[error("An identity with the name '{0}' already exists")]
    IdentityAlreadyExists(String),

    #[error(transparent)]
    Keyring(#[from] keyring::Error),
}

#[derive(Debug, clap::Parser, Clone)]
#[group(skip)]
#[allow(clippy::struct_excessive_bools)]
pub struct Cmd {
    /// Name of identity
    pub name: KeyName,

    /// Do not fund address
    #[arg(long)]
    pub no_fund: bool,

    /// Optional seed to use when generating seed phrase.
    /// Random otherwise.
    #[arg(long, conflicts_with = "default_seed")]
    pub seed: Option<String>,

    /// Output the generated identity as a secret key
    #[arg(long, short = 's')]
    pub as_secret: bool,

    /// Save in OS-specific secure store
    #[arg(long)]
    pub secure_store: bool,

    #[command(flatten)]
    pub config_locator: locator::Args,

    /// When generating a secret key, which `hd_path` should be used from the original `seed_phrase`.
    #[arg(long)]
    pub hd_path: Option<usize>,

    /// Generate the default seed phrase. Useful for testing.
    /// Equivalent to --seed 0000000000000000
    #[arg(long, short = 'd', conflicts_with = "seed")]
    pub default_seed: bool,

    #[command(flatten)]
    pub network: network::Args,

    /// Fund generated key pair
    #[arg(long, default_value = "false")]
    pub fund: bool,

    /// Overwrite existing identity if it already exists.
    #[arg(long)]
    pub overwrite: bool,
}

impl Cmd {
    pub async fn run(&self, global_args: &global::Args) -> Result<(), Error> {
        let print = Print::new(global_args.quiet);

        if self.config_locator.read_identity(&self.name).is_ok() {
            if !self.overwrite {
                return Err(Error::IdentityAlreadyExists(self.name.to_string()));
            }

            print.exclaimln(format!("Overwriting identity '{}'", &self.name.to_string()));
        }

        if !self.fund {
            print.warnln(
                "Behavior of `generate` will change in the \
            future, and it will no longer fund by default. If you want to fund please \
            provide `--fund` flag. If you don't need to fund your keys in the future, ignore this \
            warning. It can be suppressed with -q flag.",
            );
        }
        let secret = self.secret(&print)?;
        let path = self.config_locator.write_identity(&self.name, &secret)?;
        print.checkln(format!("Key saved with alias {:?} in {path:?}", self.name));

        if !self.no_fund {
            let addr = secret.public_key(self.hd_path)?;
            let network = self.network.get(&self.config_locator)?;
            network
                .fund_address(&addr)
                .await
                .map_err(|e| {
                    tracing::warn!("fund_address failed: {e}");
                })
                .unwrap_or_default();
            print.checkln(format!(
                "Account {:?} funded on {:?}",
                self.name, network.network_passphrase
            ));
        }

        Ok(())
    }

    fn secret(&self, print: &Print) -> Result<Secret, Error> {
        let seed_phrase = self.seed_phrase()?;
        if self.secure_store {
            // secure_store:org.stellar.cli:<key name>
            let entry_name_with_prefix = format!(
                "{}{}-{}",
                keyring::SECURE_STORE_ENTRY_PREFIX,
                keyring::SECURE_STORE_ENTRY_SERVICE,
                self.name
            );

            //checking that the entry name is valid before writing to the secure store
            let secret: Secret = entry_name_with_prefix.parse()?;

            if let Secret::SecureStore { entry_name } = &secret {
                Self::write_to_secure_store(entry_name, seed_phrase, print)?;
            }

            return Ok(secret);
        }
        let secret: Secret = seed_phrase.into();
        Ok(if self.as_secret {
            secret.private_key(self.hd_path)?.into()
        } else {
            secret
        })
    }

    fn seed_phrase(&self) -> Result<SeedPhrase, Error> {
        Ok(if self.default_seed {
            secret::test_seed_phrase()
        } else {
            secret::seed_phrase_from_seed(self.seed.as_deref())
        }?)
    }

    fn write_to_secure_store(
        entry_name: &String,
        seed_phrase: SeedPhrase,
        print: &Print,
    ) -> Result<(), Error> {
        print.infoln(format!("Writing to secure store: {entry_name}"));
        let entry = StellarEntry::new(entry_name)?;
        if let Ok(key) = entry.get_public_key(None) {
            print.warnln(format!("A key for {entry_name} already exists in your operating system's secure store: {key}"));
        } else {
            print.infoln(format!(
                "Saving a new key to your operating system's secure store: {entry_name}"
            ));
            entry.set_seed_phrase(seed_phrase)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::config::{address::KeyName, secret::Secret};
    use keyring::{mock, set_default_credential_builder};

    fn set_up_test() -> (super::locator::Args, super::Cmd) {
        let temp_dir = tempfile::tempdir().unwrap();
        let locator = super::locator::Args {
            global: false,
            config_dir: Some(temp_dir.path().to_path_buf()),
        };

        let cmd = super::Cmd {
            name: KeyName("test_name".to_string()),
            no_fund: true,
            seed: None,
            as_secret: false,
            secure_store: false,
            config_locator: locator.clone(),
            hd_path: None,
            default_seed: false,
            network: super::network::Args::default(),
            fund: false,
            overwrite: false,
        };

        (locator, cmd)
    }

    fn global_args() -> super::global::Args {
        super::global::Args {
            quiet: true,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_storing_secret_as_a_seed_phrase() {
        let (test_locator, cmd) = set_up_test();
        let global_args = global_args();

        let result = cmd.run(&global_args).await;
        assert!(result.is_ok());
        let identity = test_locator.read_identity("test_name").unwrap();
        assert!(matches!(identity, Secret::SeedPhrase { .. }));
    }

    #[tokio::test]
    async fn test_storing_secret_as_a_secret_key() {
        let (test_locator, mut cmd) = set_up_test();
        cmd.as_secret = true;
        let global_args = global_args();

        let result = cmd.run(&global_args).await;
        assert!(result.is_ok());
        let identity = test_locator.read_identity("test_name").unwrap();
        assert!(matches!(identity, Secret::SecretKey { .. }));
    }

    #[tokio::test]
    async fn test_storing_secret_in_secure_store() {
        set_default_credential_builder(mock::default_credential_builder());
        let (test_locator, mut cmd) = set_up_test();
        cmd.secure_store = true;
        let global_args = global_args();

        let result = cmd.run(&global_args).await;
        assert!(result.is_ok());
        let identity = test_locator.read_identity("test_name").unwrap();
        assert!(matches!(identity, Secret::SecureStore { .. }));
    }
}
