use crate::xdr::{
    self, AccountId, ContractIdPreimage, ContractIdPreimageFromAddress, Hash, HashIdPreimage,
    HashIdPreimageContractId, Limits, PublicKey, ScAddress, Uint256, WriteXdr,
};
use clap::{arg, command, Parser};
use sha2::{Digest, Sha256};

use crate::config;

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    /// ID of the Soroban contract
    #[arg(long)]
    pub salt: String,

    #[command(flatten)]
    pub config: config::Args,
}
#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    ConfigError(#[from] config::Error),
    #[error(transparent)]
    Xdr(#[from] xdr::Error),
    #[error("cannot parse salt {0}")]
    CannotParseSalt(String),
    #[error("only Ed25519 accounts are allowed")]
    OnlyEd25519AccountsAllowed,
}
impl Cmd {
    pub fn run(&self) -> Result<(), Error> {
        let salt: [u8; 32] = soroban_spec_tools::utils::padded_hex_from_str(&self.salt, 32)
            .map_err(|_| Error::CannotParseSalt(self.salt.clone()))?
            .try_into()
            .map_err(|_| Error::CannotParseSalt(self.salt.clone()))?;
        // let source_account = match self.config.source_account()? {
        //     xdr::MuxedAccount::Ed25519(uint256) => stellar_strkey::ed25519::PublicKey(uint256.0),
        //     xdr::MuxedAccount::MuxedEd25519(_) => return Err(Error::OnlyEd25519AccountsAllowed),
        // };
        let contract_id_preimage = contract_preimage(source_account.try_into()?, salt.into());
        let contract_id = get_contract_id(
            contract_id_preimage,
            &self.config.get_network()?.network_passphrase,
        )?;
        println!("{contract_id}");
        Ok(())
    }
}

pub fn contract_preimage(
    address: &ScAddress,
    salt: Hash,
) -> ContractIdPreimage {
    ContractIdPreimage::Address(ContractIdPreimageFromAddress {
        address,
        salt,
    })
}

pub fn get_contract_id(
    contract_id_preimage: ContractIdPreimage,
    network_passphrase: &str,
) -> Result<stellar_strkey::Contract, Error> {
    let network_id = network_passphrase.into();
    HashIdPreimage::ContractId(HashIdPreimageContractId {
        network_id,
        contract_id_preimage,
    })
    .try_into()
}
