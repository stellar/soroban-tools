use clap::arg;
use directories::UserDirs;
use itertools::Itertools;
use serde::de::DeserializeOwned;
use std::{
    ffi::OsStr,
    fmt::Display,
    fs::{self, create_dir_all, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    str::FromStr,
};
use stellar_strkey::{Contract, DecodeError};

use crate::{commands::HEADING_GLOBAL, utils::find_config_dir, Pwd};

use super::{
    alias,
    network::{self, Network},
    secret::Secret,
    Config,
};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error(transparent)]
    TomlSerialize(#[from] toml::ser::Error),
    #[error("Failed to find home directory")]
    HomeDirNotFound,
    #[error("Failed read current directory")]
    CurrentDirNotFound,
    #[error("Failed read current directory and no SOROBAN_CONFIG_HOME is set")]
    NoConfigEnvVar,
    #[error("Failed to create directory: {path:?}")]
    DirCreationFailed { path: PathBuf },
    #[error("Failed to read secret's file: {path}.\nProbably need to use `stellar keys add`")]
    SecretFileRead { path: PathBuf },
    #[error("Failed to read network file: {path};\nProbably need to use `stellar network add`")]
    NetworkFileRead { path: PathBuf },
    #[error("Failed to read file: {path}")]
    FileRead { path: PathBuf },
    #[error(transparent)]
    Toml(#[from] toml::de::Error),
    #[error("Secret file failed to deserialize")]
    Deserialization,
    #[error("Failed to write identity file:{filepath}: {error}")]
    IdCreationFailed { filepath: PathBuf, error: io::Error },
    #[error("Secret file failed to deserialize")]
    NetworkDeserialization,
    #[error("Failed to write network file: {0}")]
    NetworkCreationFailed(std::io::Error),
    #[error("Error Identity directory is invalid: {name}")]
    IdentityList { name: String },
    // #[error("Config file failed to deserialize")]
    // CannotReadConfigFile,
    #[error("Config file failed to serialize")]
    ConfigSerialization,
    // #[error("Config file failed write")]
    // CannotWriteConfigFile,
    #[error("XDG_CONFIG_HOME env variable is not a valid path. Got {0}")]
    XdgConfigHome(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("Failed to remove {0}: {1}")]
    ConfigRemoval(String, String),
    #[error("Failed to find config {0} for {1}")]
    ConfigMissing(String, String),
    #[error(transparent)]
    String(#[from] std::string::FromUtf8Error),
    #[error(transparent)]
    Secret(#[from] crate::config::secret::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("cannot access config dir for alias file")]
    CannotAccessConfigDir,
    #[error("cannot access alias config file (no permission or doesn't exist)")]
    CannotAccessAliasConfigFile,
    #[error("cannot parse contract ID {0}: {1}")]
    CannotParseContractId(String, DecodeError),
    #[error("contract not found: {0}")]
    ContractNotFound(String),
    #[error("Failed to read upgrade check file: {path}: {error}")]
    UpgradeCheckReadFailed { path: PathBuf, error: io::Error },
    #[error("Failed to write upgrade check file: {path}: {error}")]
    UpgradeCheckWriteFailed { path: PathBuf, error: io::Error },
    #[error("Contract alias {0}, cannot overlap with key")]
    ContractAliasCannotOverlapWithKey(String),
    #[error("Key cannot {0} cannot overlap with contract alias")]
    KeyCannotOverlapWithContractAlias(String),
}

#[derive(Debug, clap::Args, Default, Clone)]
#[group(skip)]
pub struct Args {
    /// Use global config
    #[arg(long, global = true, help_heading = HEADING_GLOBAL)]
    pub global: bool,

    /// Location of config directory, default is "."
    #[arg(long, global = true, help_heading = HEADING_GLOBAL)]
    pub config_dir: Option<PathBuf>,
}

pub enum Location {
    Local(PathBuf),
    Global(PathBuf),
}

impl Display for Location {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {:?}",
            match self {
                Location::Local(_) => "Local",
                Location::Global(_) => "Global",
            },
            self.as_ref()
        )
    }
}

impl AsRef<Path> for Location {
    fn as_ref(&self) -> &Path {
        match self {
            Location::Local(p) | Location::Global(p) => p.as_path(),
        }
    }
}

impl Location {
    #[must_use]
    pub fn wrap(&self, p: PathBuf) -> Self {
        match self {
            Location::Local(_) => Location::Local(p),
            Location::Global(_) => Location::Global(p),
        }
    }
}

impl Args {
    pub fn config_dir(&self) -> Result<PathBuf, Error> {
        if self.global {
            global_config_path()
        } else {
            self.local_config()
        }
    }

    pub fn local_and_global(&self) -> Result<[Location; 2], Error> {
        Ok([
            Location::Local(self.local_config()?),
            Location::Global(global_config_path()?),
        ])
    }

    pub fn local_config(&self) -> Result<PathBuf, Error> {
        let pwd = self.current_dir()?;
        Ok(find_config_dir(pwd.clone()).unwrap_or_else(|_| pwd.join(".stellar")))
    }

    pub fn current_dir(&self) -> Result<PathBuf, Error> {
        self.config_dir.as_ref().map_or_else(
            || std::env::current_dir().map_err(|_| Error::CurrentDirNotFound),
            |pwd| Ok(pwd.clone()),
        )
    }

    pub fn write_identity(&self, name: &str, secret: &Secret) -> Result<PathBuf, Error> {
        if let Ok(Some(_)) = self.load_contract_from_alias(name) {
            return Err(Error::KeyCannotOverlapWithContractAlias(name.to_owned()));
        }
        KeyType::Identity.write(name, secret, &self.config_dir()?)
    }

    pub fn write_network(&self, name: &str, network: &Network) -> Result<PathBuf, Error> {
        KeyType::Network.write(name, network, &self.config_dir()?)
    }

    pub fn write_default_network(&self, name: &str) -> Result<(), Error> {
        Config::new()?.set_network(name).save()
    }

    pub fn write_default_identity(&self, name: &str) -> Result<(), Error> {
        Config::new()?.set_identity(name).save()
    }

    pub fn list_identities(&self) -> Result<Vec<String>, Error> {
        Ok(KeyType::Identity
            .list_paths(&self.local_and_global()?)?
            .into_iter()
            .map(|(name, _)| name)
            .collect())
    }

    pub fn list_identities_long(&self) -> Result<Vec<(String, String)>, Error> {
        Ok(KeyType::Identity
            .list_paths(&self.local_and_global()?)
            .into_iter()
            .flatten()
            .map(|(name, location)| {
                let path = match location {
                    Location::Local(path) | Location::Global(path) => path,
                };
                (name, format!("{}", path.display()))
            })
            .collect())
    }

    pub fn list_networks(&self) -> Result<Vec<String>, Error> {
        let saved_networks = KeyType::Network
            .list_paths(&self.local_and_global()?)
            .into_iter()
            .flatten()
            .map(|x| x.0);
        let default_networks = network::DEFAULTS.keys().map(ToString::to_string);
        Ok(saved_networks.chain(default_networks).unique().collect())
    }

    pub fn list_networks_long(&self) -> Result<Vec<(String, Network, String)>, Error> {
        let saved_networks = KeyType::Network
            .list_paths(&self.local_and_global()?)
            .into_iter()
            .flatten()
            .filter_map(|(name, location)| {
                Some((
                    name,
                    KeyType::read_from_path::<Network>(location.as_ref()).ok()?,
                    location.to_string(),
                ))
            });
        let default_networks = network::DEFAULTS
            .into_iter()
            .map(|(name, network)| ((*name).to_string(), network.into(), "Default".to_owned()));
        Ok(saved_networks.chain(default_networks).collect())
    }

    pub fn read_identity(&self, name: &str) -> Result<Secret, Error> {
        Ok(KeyType::Identity
            .read_with_global(name, &self.local_config()?)
            .or_else(|_| name.parse())?)
    }

    pub fn key(&self, key_or_name: &str) -> Result<Secret, Error> {
        if let Ok(signer) = key_or_name.parse::<Secret>() {
            Ok(signer)
        } else {
            self.read_identity(key_or_name)
        }
    }

    pub fn read_network(&self, name: &str) -> Result<Network, Error> {
        let res = KeyType::Network.read_with_global(name, &self.local_config()?);
        if let Err(Error::ConfigMissing(_, _)) = &res {
            let Some(network) = network::DEFAULTS.get(name) else {
                return res;
            };
            return Ok(network.into());
        }
        res
    }

    pub fn remove_identity(&self, name: &str) -> Result<(), Error> {
        KeyType::Identity.remove(name, &self.config_dir()?)
    }

    pub fn remove_network(&self, name: &str) -> Result<(), Error> {
        KeyType::Network.remove(name, &self.config_dir()?)
    }

    fn load_contract_from_alias(&self, alias: &str) -> Result<Option<alias::Data>, Error> {
        let path = self.alias_path(alias)?;

        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(path)?;
        let data: alias::Data = serde_json::from_str(&content).unwrap_or_default();

        Ok(Some(data))
    }

    fn alias_path(&self, alias: &str) -> Result<PathBuf, Error> {
        let file_name = format!("{alias}.json");
        let config_dir = self.config_dir()?;
        Ok(config_dir.join("contract-ids").join(file_name))
    }

    pub fn save_contract_id(
        &self,
        network_passphrase: &str,
        contract_id: &stellar_strkey::Contract,
        alias: &str,
    ) -> Result<(), Error> {
        if self.read_identity(alias).is_ok() {
            return Err(Error::ContractAliasCannotOverlapWithKey(alias.to_owned()));
        }
        let path = self.alias_path(alias)?;
        let dir = path.parent().ok_or(Error::CannotAccessConfigDir)?;

        create_dir_all(dir).map_err(|_| Error::CannotAccessConfigDir)?;

        let content = fs::read_to_string(&path).unwrap_or_default();
        let mut data: alias::Data = serde_json::from_str(&content).unwrap_or_default();

        let mut to_file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)?;

        data.ids
            .insert(network_passphrase.into(), contract_id.to_string());

        let content = serde_json::to_string(&data)?;

        Ok(to_file.write_all(content.as_bytes())?)
    }

    pub fn remove_contract_id(&self, network_passphrase: &str, alias: &str) -> Result<(), Error> {
        let path = self.alias_path(alias)?;

        if !path.is_file() {
            return Err(Error::CannotAccessAliasConfigFile);
        }

        let content = fs::read_to_string(&path).unwrap_or_default();
        let mut data: alias::Data = serde_json::from_str(&content).unwrap_or_default();

        let mut to_file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)?;

        data.ids.remove::<str>(network_passphrase);

        let content = serde_json::to_string(&data)?;

        Ok(to_file.write_all(content.as_bytes())?)
    }

    pub fn get_contract_id(
        &self,
        alias: &str,
        network_passphrase: &str,
    ) -> Result<Option<Contract>, Error> {
        let Some(alias_data) = self.load_contract_from_alias(alias)? else {
            return Ok(None);
        };

        alias_data
            .ids
            .get(network_passphrase)
            .map(|id| id.parse())
            .transpose()
            .map_err(|e| Error::CannotParseContractId(alias.to_owned(), e))
    }

    pub fn resolve_contract_id(
        &self,
        alias_or_contract_id: &str,
        network_passphrase: &str,
    ) -> Result<Contract, Error> {
        let Some(contract) = self.get_contract_id(alias_or_contract_id, network_passphrase)? else {
            return alias_or_contract_id
                .parse()
                .map_err(|e| Error::CannotParseContractId(alias_or_contract_id.to_owned(), e));
        };
        Ok(contract)
    }
}

impl Pwd for Args {
    fn set_pwd(&mut self, pwd: &Path) {
        self.config_dir = Some(pwd.to_path_buf());
    }
}

pub fn ensure_directory(dir: PathBuf) -> Result<PathBuf, Error> {
    let parent = dir.parent().ok_or(Error::HomeDirNotFound)?;
    std::fs::create_dir_all(parent).map_err(|_| dir_creation_failed(parent))?;
    Ok(dir)
}

fn dir_creation_failed(p: &Path) -> Error {
    Error::DirCreationFailed {
        path: p.to_path_buf(),
    }
}

fn read_dir(dir: &Path) -> Result<Vec<(String, PathBuf)>, Error> {
    let contents = std::fs::read_dir(dir)?;
    let mut res = vec![];
    for entry in contents.filter_map(Result::ok) {
        let path = entry.path();
        if let Some("toml") = path.extension().and_then(OsStr::to_str) {
            if let Some(os_str) = path.file_stem() {
                res.push((os_str.to_string_lossy().trim().to_string(), path));
            }
        }
    }
    res.sort();
    Ok(res)
}

pub enum KeyType {
    Identity,
    Network,
}

impl Display for KeyType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                KeyType::Identity => "identity",
                KeyType::Network => "network",
            }
        )
    }
}

impl KeyType {
    pub fn read<T: DeserializeOwned>(&self, key: &str, pwd: &Path) -> Result<T, Error> {
        let path = self.path(pwd, key);
        Self::read_from_path(&path)
    }

    pub fn read_from_path<T: DeserializeOwned>(path: &Path) -> Result<T, Error> {
        let data = fs::read_to_string(path).map_err(|_| Error::NetworkFileRead {
            path: path.to_path_buf(),
        })?;
        Ok(toml::from_str(&data)?)
    }

    pub fn read_with_global<T: DeserializeOwned>(&self, key: &str, pwd: &Path) -> Result<T, Error> {
        for path in [pwd, global_config_path()?.as_path()] {
            match self.read(key, path) {
                Ok(t) => return Ok(t),
                _ => continue,
            }
        }
        Err(Error::ConfigMissing(self.to_string(), key.to_string()))
    }

    pub fn write<T: serde::Serialize>(
        &self,
        key: &str,
        value: &T,
        pwd: &Path,
    ) -> Result<PathBuf, Error> {
        let filepath = ensure_directory(self.path(pwd, key))?;
        let data = toml::to_string(value).map_err(|_| Error::ConfigSerialization)?;
        std::fs::write(&filepath, data).map_err(|error| Error::IdCreationFailed {
            filepath: filepath.clone(),
            error,
        })?;
        Ok(filepath)
    }

    fn root(&self, pwd: &Path) -> PathBuf {
        pwd.join(self.to_string())
    }

    fn path(&self, pwd: &Path, key: &str) -> PathBuf {
        let mut path = self.root(pwd).join(key);
        path.set_extension("toml");
        path
    }

    pub fn list_paths(&self, paths: &[Location]) -> Result<Vec<(String, Location)>, Error> {
        Ok(paths
            .iter()
            .flat_map(|p| self.list(p).unwrap_or_default())
            .collect())
    }

    pub fn list(&self, pwd: &Location) -> Result<Vec<(String, Location)>, Error> {
        let path = self.root(pwd.as_ref());
        if path.exists() {
            let mut files = read_dir(&path)?;
            files.sort();

            Ok(files
                .into_iter()
                .map(|(name, p)| (name, pwd.wrap(p)))
                .collect())
        } else {
            Ok(vec![])
        }
    }

    pub fn remove(&self, key: &str, pwd: &Path) -> Result<(), Error> {
        let path = self.path(pwd, key);
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|_| Error::ConfigRemoval(self.to_string(), key.to_string()))
        } else {
            Ok(())
        }
    }
}

pub fn global_config_path() -> Result<PathBuf, Error> {
    let config_dir = if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from_str(&config_home).map_err(|_| Error::XdgConfigHome(config_home))?
    } else {
        UserDirs::new()
            .ok_or(Error::HomeDirNotFound)?
            .home_dir()
            .join(".config")
    };

    let soroban_dir = config_dir.join("soroban");
    let stellar_dir = config_dir.join("stellar");
    let soroban_exists = soroban_dir.exists();
    let stellar_exists = stellar_dir.exists();

    if stellar_exists && soroban_exists {
        tracing::warn!("the .stellar and .soroban config directories exist at path {config_dir:?}, using the .stellar");
    }

    if stellar_exists {
        return Ok(stellar_dir);
    }

    if soroban_exists {
        return Ok(soroban_dir);
    }

    Ok(stellar_dir)
}

pub fn config_file() -> Result<PathBuf, Error> {
    Ok(global_config_path()?.join("config.toml"))
}
