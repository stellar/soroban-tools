use ed25519_dalek::Signer;
use keyring::Entry;
use sep5::seed_phrase::SeedPhrase;
use zeroize::Zeroize;

pub(crate) const SECURE_STORE_ENTRY_PREFIX: &str = "secure_store:";
pub(crate) const SECURE_STORE_ENTRY_SERVICE: &str = "org.stellar.cli";

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Keyring(#[from] keyring::Error),
    #[error(transparent)]
    Sep5(#[from] sep5::error::Error),
}

pub struct StellarEntry {
    keyring: Entry,
}

impl StellarEntry {
    pub fn new(name: &str) -> Result<Self, Error> {
        Ok(StellarEntry {
            keyring: Entry::new(name, &whoami::username())?,
        })
    }

    pub fn set_seed_phrase(&self, seed_phrase: SeedPhrase) -> Result<(), Error> {
        let mut data = seed_phrase.seed_phrase.into_phrase();
        self.keyring.set_password(&data)?;
        data.zeroize();
        Ok(())
    }

    fn get_seed_phrase(&self) -> Result<SeedPhrase, Error> {
        Ok(self.keyring.get_password()?.parse()?)
    }

    fn use_key<T>(
        &self,
        f: impl FnOnce(ed25519_dalek::SigningKey) -> Result<T, Error>,
        hd_path: Option<usize>,
    ) -> Result<T, Error> {
        // The underlying Mnemonic type is zeroized when dropped
        let mut key_bytes: [u8; 32] = {
            self.get_seed_phrase()?
                .from_path_index(hd_path.unwrap_or_default(), None)?
                .private()
                .0
        };
        let result = {
            // Use this scope to ensure the keypair is zeroized when dropped
            let keypair = ed25519_dalek::SigningKey::from_bytes(&key_bytes);
            f(keypair)?
        };
        key_bytes.zeroize();
        Ok(result)
    }

    pub fn get_public_key(
        &self,
        hd_path: Option<usize>,
    ) -> Result<stellar_strkey::ed25519::PublicKey, Error> {
        self.use_key(
            |keypair| {
                Ok(stellar_strkey::ed25519::PublicKey(
                    *keypair.verifying_key().as_bytes(),
                ))
            },
            hd_path,
        )
    }

    pub fn sign_data(&self, data: &[u8], hd_path: Option<usize>) -> Result<Vec<u8>, Error> {
        self.use_key(
            |keypair| {
                let signature = keypair.sign(data);
                Ok(signature.to_bytes().to_vec())
            },
            hd_path,
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use keyring::{mock, set_default_credential_builder};

    #[test]
    fn test_get_password() {
        set_default_credential_builder(mock::default_credential_builder());

        let seed_phrase = crate::config::secret::seed_phrase_from_seed(None).unwrap();
        let seed_phrase_clone = seed_phrase.clone();

        let entry = StellarEntry::new("test").unwrap();

        // set the seed phrase
        let set_seed_phrase_result = entry.set_seed_phrase(seed_phrase);
        assert!(set_seed_phrase_result.is_ok());

        // get_seed_phrase should return the same seed phrase we set
        let get_seed_phrase_result = entry.get_seed_phrase();
        assert!(get_seed_phrase_result.is_ok());
        assert_eq!(
            seed_phrase_clone.phrase(),
            get_seed_phrase_result.unwrap().phrase()
        );
    }

    #[test]
    fn test_get_public_key() {
        set_default_credential_builder(mock::default_credential_builder());

        let seed_phrase = crate::config::secret::seed_phrase_from_seed(None).unwrap();
        let public_key = seed_phrase.from_path_index(0, None).unwrap().public().0;

        let entry = StellarEntry::new("test").unwrap();

        // set the seed_phrase
        let set_seed_phrase_result = entry.set_seed_phrase(seed_phrase);
        assert!(set_seed_phrase_result.is_ok());

        // confirm that we can get the public key from the entry and that it matches the one we set
        let get_public_key_result = entry.get_public_key(None);
        assert!(get_public_key_result.is_ok());
        assert_eq!(public_key, get_public_key_result.unwrap().0);
    }

    #[test]
    fn test_sign_data() {
        set_default_credential_builder(mock::default_credential_builder());

        //create a seed phrase
        let seed_phrase = crate::config::secret::seed_phrase_from_seed(None).unwrap();

        // create a keyring entry and set the seed_phrase
        let entry = StellarEntry::new("test").unwrap();
        entry.set_seed_phrase(seed_phrase).unwrap();

        let tx_xdr = r"AAAAAgAAAADh6eOnZEq1xQgKioffuH7/8D8x8+OdGFEkiYC6QKMWzQAAAGQAAACuAAAAAQAAAAAAAAAAAAAAAQAAAAAAAAAYAAAAAQAAAAAAAAAAAAAAAOHp46dkSrXFCAqKh9+4fv/wPzHz450YUSSJgLpAoxbNoFT1s8jZPCv9IJ2DsqGTA8pOtavv58JF53aDycpRPcEAAAAA+N2m5zc3EfWUmLvigYPOHKXhSy8OrWfVibc6y6PrQoYAAAAAAAAAAAAAAAA";

        let sign_tx_env_result = entry.sign_data(tx_xdr.as_bytes(), None);
        assert!(sign_tx_env_result.is_ok());
    }
}
