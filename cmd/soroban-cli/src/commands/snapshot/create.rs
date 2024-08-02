use async_compression::tokio::bufread::GzipDecoder;
use bytesize::ByteSize;
use clap::{arg, Parser, ValueEnum};
use futures::{StreamExt, TryStreamExt};
use http::Uri;
use humantime::format_duration;
use itertools::{Either, Itertools};
use sha2::{Digest, Sha256};
use soroban_ledger_snapshot::LedgerSnapshot;
use std::{
    collections::HashSet,
    fs::{self},
    io::{self},
    path::PathBuf,
    str::FromStr,
    time::{Duration, Instant},
};
use stellar_xdr::curr::{
    AccountId, BucketEntry, ConfigSettingEntry, ConfigSettingId, ContractExecutable, Frame, Hash,
    LedgerEntry, LedgerEntryData, LedgerKey, LedgerKeyAccount, LedgerKeyClaimableBalance,
    LedgerKeyConfigSetting, LedgerKeyContractCode, LedgerKeyContractData, LedgerKeyData,
    LedgerKeyLiquidityPool, LedgerKeyOffer, LedgerKeyTrustLine, LedgerKeyTtl, Limited, Limits,
    ReadXdr, ScAddress, ScContractInstance, ScVal,
};
use tokio::fs::OpenOptions;

use soroban_env_host::xdr::{self};

use crate::{
    commands::{config::data, HEADING_RPC},
    config::{self, locator, network::passphrase},
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, ValueEnum)]
pub enum Output {
    Json,
}

impl Default for Output {
    fn default() -> Self {
        Self::Json
    }
}

fn default_out_path() -> PathBuf {
    PathBuf::new().join("snapshot.json")
}

#[derive(Parser, Debug, Clone)]
#[group(skip)]
#[command(arg_required_else_help = true)]
pub struct Cmd {
    /// The ledger sequence number to snapshot. Defaults to latest history archived ledger.
    #[arg(long)]
    ledger: Option<u32>,
    /// Account or contract address to include in the snapshot.
    #[arg(long = "address", help_heading = "Filter Options")]
    address: Vec<ScAddress>,
    /// WASM hashes to include in the snapshot.
    #[arg(long = "wasm-hash", help_heading = "Filter Options")]
    wasm_hashes: Vec<Hash>,
    /// Format of the out file.
    #[arg(long)]
    output: Output,
    /// Out path that the snapshot is written to.
    #[arg(long, default_value=default_out_path().into_os_string())]
    out: PathBuf,
    #[command(flatten)]
    locator: locator::Args,
    #[command(flatten)]
    network: config::network::Args,
    /// Archive URL
    #[arg(long, help_heading = HEADING_RPC, env = "STELLAR_ARCHIVE_URL")]
    archive_url: Option<Uri>,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("wasm hash invalid: {0}")]
    WasmHashInvalid(String),
    #[error("downloading history: {0}")]
    DownloadingHistory(hyper::Error),
    #[error("downloading history: got status code {0}")]
    DownloadingHistoryGotStatusCode(hyper::StatusCode),
    #[error("json decoding history: {0}")]
    JsonDecodingHistory(serde_json::Error),
    #[error("opening cached bucket to read: {0}")]
    ReadOpeningCachedBucket(io::Error),
    #[error("parsing bucket url: {0}")]
    ParsingBucketUrl(http::uri::InvalidUri),
    #[error("getting bucket: {0}")]
    GettingBucket(hyper::Error),
    #[error("getting bucket: got status code {0}")]
    GettingBucketGotStatusCode(hyper::StatusCode),
    #[error("opening cached bucket to write: {0}")]
    WriteOpeningCachedBucket(io::Error),
    #[error("streaming bucket: {0}")]
    StreamingBucket(io::Error),
    #[error("read XDR frame bucket entry: {0}")]
    ReadXdrFrameBucketEntry(xdr::Error),
    #[error("renaming temporary downloaded file to final destination: {0}")]
    RenameDownloadFile(io::Error),
    #[error("getting bucket directory: {0}")]
    GetBucketDir(data::Error),
    #[error("reading history http stream: {0}")]
    ReadHistoryHttpStream(hyper::Error),
    #[error("writing ledger snapshot: {0}")]
    WriteLedgerSnapshot(soroban_ledger_snapshot::Error),
    #[error(transparent)]
    Join(#[from] tokio::task::JoinError),
    #[error(transparent)]
    Network(#[from] config::network::Error),
    #[error(transparent)]
    Locator(#[from] locator::Error),
    #[error(transparent)]
    Config(#[from] config::Error),
    #[error("archive url not configured")]
    ArchiveUrlNotConfigured,
}

/// Checkpoint frequency is usually 64 ledgers, but in local test nets it'll
/// often by 8. There's no way to simply detect what frequency to expect ledgers
/// at, so it is hardcoded at 64, and this value is used only to help the user
/// select good ledger numbers when they select one that doesn't exist.
const CHECKPOINT_FREQUENCY: u32 = 64;

impl Cmd {
    #[allow(clippy::too_many_lines)]
    pub async fn run(&self) -> Result<(), Error> {
        let start = Instant::now();

        let archive_url = self.archive_url()?;
        let history = get_history(&archive_url, self.ledger).await?;

        let ledger = history.current_ledger;
        let network_passphrase = &history.network_passphrase;
        let network_id = Sha256::digest(network_passphrase);
        println!("ℹ️  Ledger: {ledger}");
        println!("ℹ️  Network Passphrase: {network_passphrase}");
        println!("ℹ️  Network ID: {}", hex::encode(network_id));

        // Prepare a flat list of buckets to read. They'll be ordered by their
        // level so that they can iterated higher level to lower level.
        let buckets = history
            .current_buckets
            .iter()
            .flat_map(|h| [h.curr.clone(), h.snap.clone()])
            .filter(|b| b != "0000000000000000000000000000000000000000000000000000000000000000")
            .collect::<Vec<_>>();

        // Pre-cache the buckets.
        for (i, bucket) in buckets.iter().enumerate() {
            cache_bucket(&archive_url, i, bucket).await?;
        }

        // The snapshot is what will be written to file at the end. Fields will
        // be updated while parsing the history archive.
        let mut snapshot = LedgerSnapshot {
            // TODO: Update more of the fields.
            protocol_version: 0,
            sequence_number: ledger,
            timestamp: 0,
            network_id: network_id.into(),
            base_reserve: 1,
            min_persistent_entry_ttl: 0,
            min_temp_entry_ttl: 0,
            max_entry_ttl: 0,
            ledger_entries: Vec::new(),
        };

        // Track ledger keys seen, so that we can ignore old versions of
        // entries. Entries can appear in both higher level and lower level
        // buckets, and to get the latest version of the entry the version in
        // the higher level bucket should be used.
        let mut seen = HashSet::new();

        let (account_ids, contract_ids) = self.addresses();
        let wasm_hashes = HashSet::<&Hash>::from_iter(&self.wasm_hashes);
        let mut next_wasm_hashes = HashSet::<Hash>::new();

        // Search the buckets.
        println!(
            "ℹ️  Searching for {} accounts, {} contracts, {} wasms",
            account_ids.len(),
            contract_ids.len(),
            wasm_hashes.len()
        );
        for (i, bucket) in buckets.iter().enumerate() {
            // Defined where the bucket will be read from, either from cache on
            // disk, or streamed from the archive.
            let cache_path = cache_bucket(&archive_url, i, bucket).await?;
            let file = std::fs::OpenOptions::new()
                .read(true)
                .open(&cache_path)
                .map_err(Error::ReadOpeningCachedBucket)?;
            print!("🔎 Searching bucket {i} {bucket}");
            if let Ok(metadata) = file.metadata() {
                print!(" ({})", ByteSize(metadata.len()));
            }
            println!();

            // Stream the bucket entries from the bucket, identifying
            // entries that match the filters, and including only the
            // entries that match in the snapshot.
            let limited = &mut Limited::new(file, Limits::none());
            let entries = Frame::<BucketEntry>::read_xdr_iter(limited);
            let mut count_saved = 0;
            for entry in entries {
                let Frame(entry) = entry.map_err(Error::ReadXdrFrameBucketEntry)?;
                let (key, val) = match entry {
                    BucketEntry::Liveentry(l) | BucketEntry::Initentry(l) => {
                        let k = data_into_key(&l);
                        (k, Some(l))
                    }
                    BucketEntry::Deadentry(k) => (k, None),
                    BucketEntry::Metaentry(m) => {
                        snapshot.protocol_version = m.ledger_version;
                        continue;
                    }
                };
                if seen.contains(&key) {
                    continue;
                }
                let keep = match &key {
                    LedgerKey::Account(k) => account_ids.contains(&k.account_id),
                    LedgerKey::Trustline(k) => account_ids.contains(&k.account_id),
                    LedgerKey::ContractData(k) => contract_ids.contains(&k.contract),
                    LedgerKey::ContractCode(e) => wasm_hashes.contains(&e.hash),
                    _ => false,
                };
                if !keep {
                    continue;
                }
                seen.insert(key.clone());
                let Some(val) = val else { continue };
                match &val.data {
                    LedgerEntryData::ContractData(e) => {
                        // If a contract instance references contract
                        // executable stored in another ledger entry, add
                        // that ledger entry to the filter so that Wasm for
                        // any filtered contract is collected too in the
                        // second pass.
                        if keep && e.key == ScVal::LedgerKeyContractInstance {
                            if let ScVal::ContractInstance(ScContractInstance {
                                executable: ContractExecutable::Wasm(hash),
                                ..
                            }) = &e.val
                            {
                                if !wasm_hashes.contains(hash) {
                                    next_wasm_hashes.insert(hash.clone());
                                    println!("ℹ️  Adding wasm {} to search", hex::encode(hash));
                                }
                            }
                        }
                        keep
                    }
                    _ => false,
                };
                snapshot
                    .ledger_entries
                    .push((Box::new(key), (Box::new(val), Some(u32::MAX))));
                count_saved += 1;
            }
            if count_saved > 0 {
                println!("ℹ️  Found {count_saved} entries");
            }
        }
        seen.clear();

        // Parse the buckets a second time if we found wasms in the first pass
        // that should be included.
        println!(
            "ℹ️  Searching for {} additional wasms",
            next_wasm_hashes.len()
        );
        for (i, bucket) in buckets.iter().enumerate() {
            if next_wasm_hashes.is_empty() {
                break;
            }
            // Defined where the bucket will be read from, either from cache on
            // disk, or streamed from the archive.
            let cache_path = cache_bucket(&archive_url, i, bucket).await?;
            let file = std::fs::OpenOptions::new()
                .read(true)
                .open(&cache_path)
                .map_err(Error::ReadOpeningCachedBucket)?;
            print!("🔎 Searching bucket {i} {bucket}");
            if let Ok(metadata) = file.metadata() {
                print!(" ({})", ByteSize(metadata.len()));
            }
            println!();

            // Stream the bucket entries from the bucket, identifying
            // entries that match the filters, and including only the
            // entries that match in the snapshot.
            let limited = &mut Limited::new(file, Limits::none());
            let entries = Frame::<BucketEntry>::read_xdr_iter(limited);
            let mut count_saved = 0;
            for entry in entries {
                if next_wasm_hashes.is_empty() {
                    break;
                }
                let Frame(entry) = entry.map_err(Error::ReadXdrFrameBucketEntry)?;
                let (key, val) = match entry {
                    BucketEntry::Liveentry(l) | BucketEntry::Initentry(l) => {
                        let k = data_into_key(&l);
                        (k, Some(l))
                    }
                    BucketEntry::Deadentry(k) => (k, None),
                    BucketEntry::Metaentry(_) => continue,
                };
                let keep = match &key {
                    LedgerKey::ContractCode(e) => next_wasm_hashes.remove(&e.hash),
                    _ => false,
                };
                if !keep {
                    continue;
                }
                let Some(val) = val else { continue };
                snapshot
                    .ledger_entries
                    .push((Box::new(key), (Box::new(val), Some(u32::MAX))));
                count_saved += 1;
            }
            if count_saved > 0 {
                println!("ℹ️  Found {count_saved} entries");
            }
        }

        // Write the snapshot to file.
        snapshot
            .write_file(&self.out)
            .map_err(Error::WriteLedgerSnapshot)?;
        println!(
            "💾 Saved {} entries to {:?}",
            snapshot.ledger_entries.len(),
            self.out
        );

        let duration = Duration::from_secs(start.elapsed().as_secs());
        println!("✅ Completed in {}", format_duration(duration));

        Ok(())
    }

    fn addresses(&self) -> (HashSet<AccountId>, HashSet<ScAddress>) {
        self.address.iter().cloned().partition_map(|a| match a {
            ScAddress::Account(account_id) => Either::Left(account_id),
            ScAddress::Contract(_) => Either::Right(a),
        })
    }

    fn archive_url(&self) -> Result<http::Uri, Error> {
        // Return the configured archive URL, or if one is not configured, guess
        // at an appropriate archive URL given the network passphrase.
        self.archive_url
            .clone()
            .or_else(|| {
                self.network.get(&self.locator).ok().and_then(|network| {
                    match network.network_passphrase.as_str() {
                        passphrase::MAINNET => {
                            Some("https://history.stellar.org/prd/core-live/core_live_001")
                        }
                        passphrase::TESTNET => {
                            Some("https://history.stellar.org/prd/core-testnet/core_testnet_001")
                        }
                        passphrase::FUTURENET => Some("https://history-futurenet.stellar.org"),
                        passphrase::LOCAL => Some("http://localhost:8000/archive"),
                        _ => None,
                    }
                    .map(|s| Uri::from_str(s).expect("archive url valid"))
                })
            })
            .ok_or(Error::ArchiveUrlNotConfigured)
    }
}

async fn get_history(archive_url: &Uri, ledger: Option<u32>) -> Result<History, Error> {
    let archive_url = archive_url.to_string();
    let archive_url = archive_url.strip_suffix("/").unwrap_or(&archive_url);
    let history_url = if let Some(ledger) = ledger {
        let ledger_hex = format!("{ledger:08x}");
        let ledger_hex_0 = &ledger_hex[0..=1];
        let ledger_hex_1 = &ledger_hex[2..=3];
        let ledger_hex_2 = &ledger_hex[4..=5];
        format!("{archive_url}/history/{ledger_hex_0}/{ledger_hex_1}/{ledger_hex_2}/history-{ledger_hex}.json")
    } else {
        format!("{archive_url}/.well-known/stellar-history.json")
    };
    let history_url = Uri::from_str(&history_url).unwrap();

    println!("🌎 Downloading history {history_url}");
    let https = hyper_tls::HttpsConnector::new();
    let response = hyper::Client::builder()
        .build::<_, hyper::Body>(https)
        .get(history_url)
        .await
        .map_err(Error::DownloadingHistory)?;
    if !response.status().is_success() {
        // Check ledger is a checkpoint ledger and available in archives.
        if let Some(ledger) = ledger {
            let ledger_offset = (ledger + 1) % CHECKPOINT_FREQUENCY;
            if ledger_offset != 0 {
                println!(
                    "ℹ️  Ledger {ledger} may not be a checkpoint ledger, try {} or {}",
                    ledger - ledger_offset,
                    ledger + (CHECKPOINT_FREQUENCY - ledger_offset),
                );
            }
        }
        return Err(Error::DownloadingHistoryGotStatusCode(response.status()));
    }
    let body = hyper::body::to_bytes(response.into_body())
        .await
        .map_err(Error::ReadHistoryHttpStream)?;
    serde_json::from_slice::<History>(&body).map_err(Error::JsonDecodingHistory)
}

async fn cache_bucket(
    archive_url: &Uri,
    bucket_index: usize,
    bucket: &str,
) -> Result<PathBuf, Error> {
    let bucket_dir = data::bucket_dir().map_err(Error::GetBucketDir)?;
    let cache_path = bucket_dir.join(format!("bucket-{bucket}.xdr"));
    if !cache_path.exists() {
        let bucket_0 = &bucket[0..=1];
        let bucket_1 = &bucket[2..=3];
        let bucket_2 = &bucket[4..=5];
        let bucket_url =
            format!("{archive_url}/bucket/{bucket_0}/{bucket_1}/{bucket_2}/bucket-{bucket}.xdr.gz");
        print!("🪣  Downloading bucket {bucket_index} {bucket}");
        let bucket_url = Uri::from_str(&bucket_url).map_err(Error::ParsingBucketUrl)?;
        let https = hyper_tls::HttpsConnector::new();
        let response = hyper::Client::builder()
            .build::<_, hyper::Body>(https)
            .get(bucket_url)
            .await
            .map_err(Error::GettingBucket)?;
        if !response.status().is_success() {
            println!();
            return Err(Error::GettingBucketGotStatusCode(response.status()));
        }
        if let Some(val) = response.headers().get("Content-Length") {
            if let Ok(str) = val.to_str() {
                if let Ok(len) = str.parse::<u64>() {
                    print!(" ({})", ByteSize(len));
                }
            }
        }
        println!();
        let read = response
            .into_body()
            .map(|result| result.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e)))
            .into_async_read();
        let read = tokio_util::compat::FuturesAsyncReadCompatExt::compat(read);
        let mut read = GzipDecoder::new(read);
        let dl_path = cache_path.with_extension("dl");
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&dl_path)
            .await
            .map_err(Error::WriteOpeningCachedBucket)?;
        tokio::io::copy(&mut read, &mut file)
            .await
            .map_err(Error::StreamingBucket)?;
        fs::rename(&dl_path, &cache_path).map_err(Error::RenameDownloadFile)?;
    }
    Ok(cache_path)
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct History {
    current_ledger: u32,
    current_buckets: Vec<HistoryBucket>,
    network_passphrase: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct HistoryBucket {
    curr: String,
    snap: String,
}

fn data_into_key(d: &LedgerEntry) -> LedgerKey {
    // TODO: Move this function into stellar-xdr.
    match &d.data {
        LedgerEntryData::Account(e) => LedgerKey::Account(LedgerKeyAccount {
            account_id: e.account_id.clone(),
        }),
        LedgerEntryData::Trustline(e) => LedgerKey::Trustline(LedgerKeyTrustLine {
            account_id: e.account_id.clone(),
            asset: e.asset.clone(),
        }),
        LedgerEntryData::Offer(e) => LedgerKey::Offer(LedgerKeyOffer {
            seller_id: e.seller_id.clone(),
            offer_id: e.offer_id,
        }),
        LedgerEntryData::Data(e) => LedgerKey::Data(LedgerKeyData {
            account_id: e.account_id.clone(),
            data_name: e.data_name.clone(),
        }),
        LedgerEntryData::ClaimableBalance(e) => {
            LedgerKey::ClaimableBalance(LedgerKeyClaimableBalance {
                balance_id: e.balance_id.clone(),
            })
        }
        LedgerEntryData::LiquidityPool(e) => LedgerKey::LiquidityPool(LedgerKeyLiquidityPool {
            liquidity_pool_id: e.liquidity_pool_id.clone(),
        }),
        LedgerEntryData::ContractData(e) => LedgerKey::ContractData(LedgerKeyContractData {
            contract: e.contract.clone(),
            key: e.key.clone(),
            durability: e.durability,
        }),
        LedgerEntryData::ContractCode(e) => LedgerKey::ContractCode(LedgerKeyContractCode {
            hash: e.hash.clone(),
        }),
        LedgerEntryData::ConfigSetting(e) => LedgerKey::ConfigSetting(LedgerKeyConfigSetting {
            config_setting_id: match e {
                ConfigSettingEntry::ContractMaxSizeBytes(_) => {
                    ConfigSettingId::ContractMaxSizeBytes
                }
                ConfigSettingEntry::ContractComputeV0(_) => ConfigSettingId::ContractComputeV0,
                ConfigSettingEntry::ContractLedgerCostV0(_) => {
                    ConfigSettingId::ContractLedgerCostV0
                }
                ConfigSettingEntry::ContractHistoricalDataV0(_) => {
                    ConfigSettingId::ContractHistoricalDataV0
                }
                ConfigSettingEntry::ContractEventsV0(_) => ConfigSettingId::ContractEventsV0,
                ConfigSettingEntry::ContractBandwidthV0(_) => ConfigSettingId::ContractBandwidthV0,
                ConfigSettingEntry::ContractCostParamsCpuInstructions(_) => {
                    ConfigSettingId::ContractCostParamsCpuInstructions
                }
                ConfigSettingEntry::ContractCostParamsMemoryBytes(_) => {
                    ConfigSettingId::ContractCostParamsMemoryBytes
                }
                ConfigSettingEntry::ContractDataKeySizeBytes(_) => {
                    ConfigSettingId::ContractDataKeySizeBytes
                }
                ConfigSettingEntry::ContractDataEntrySizeBytes(_) => {
                    ConfigSettingId::ContractDataEntrySizeBytes
                }
                ConfigSettingEntry::StateArchival(_) => ConfigSettingId::StateArchival,
                ConfigSettingEntry::ContractExecutionLanes(_) => {
                    ConfigSettingId::ContractExecutionLanes
                }
                ConfigSettingEntry::BucketlistSizeWindow(_) => {
                    ConfigSettingId::BucketlistSizeWindow
                }
                ConfigSettingEntry::EvictionIterator(_) => ConfigSettingId::EvictionIterator,
            },
        }),
        LedgerEntryData::Ttl(e) => LedgerKey::Ttl(LedgerKeyTtl {
            key_hash: e.key_hash.clone(),
        }),
    }
}
