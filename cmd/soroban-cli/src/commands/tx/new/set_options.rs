use clap::{command, Parser};

use crate::{commands::tx, xdr};

#[derive(Parser, Debug, Clone)]
#[group(skip)]
pub struct Cmd {
    #[command(flatten)]
    pub tx: tx::Args,
    #[clap(flatten)]
    pub op: Args,
}

#[derive(Debug, clap::Args, Clone)]
#[allow(clippy::struct_excessive_bools, clippy::doc_markdown)]
pub struct Args {
    #[arg(long)]
    /// Account of the inflation destination.
    pub inflation_dest: Option<xdr::AccountId>,
    #[arg(long)]
    /// A number from 0-255 (inclusive) representing the weight of the master key. If the weight of the master key is updated to 0, it is effectively disabled.
    pub master_weight: Option<u8>,
    #[arg(long)]
    /// A number from 0-255 (inclusive) representing the threshold this account sets on all operations it performs that have a low threshold.
    /// https://developers.stellar.org/docs/learn/encyclopedia/security/signatures-multisig#multisig
    pub low_threshold: Option<u8>,
    #[arg(long)]
    /// A number from 0-255 (inclusive) representing the threshold this account sets on all operations it performs that have a medium threshold.
    /// https://developers.stellar.org/docs/learn/encyclopedia/security/signatures-multisig#multisig
    pub med_threshold: Option<u8>,
    #[arg(long)]
    /// A number from 0-255 (inclusive) representing the threshold this account sets on all operations it performs that have a high threshold.
    /// https://developers.stellar.org/docs/learn/encyclopedia/security/signatures-multisig#multisig
    pub high_threshold: Option<u8>,
    #[arg(long)]
    /// Sets the home domain of an account. See https://developers.stellar.org/docs/learn/encyclopedia/network-configuration/federation.
    pub home_domain: Option<xdr::StringM<32>>,
    #[arg(long, requires = "signer_weight")]
    /// Add, update, or remove a signer from an account.
    pub signer: Option<xdr::SignerKey>,
    #[arg(long = "signer-weight", requires = "signer")]
    /// Signer weight is a number from 0-255 (inclusive). The signer is deleted if the weight is 0.
    pub signer_weight: Option<u8>,
    #[arg(long, conflicts_with = "clear_required")]
    /// When enabled, an issuer must approve an account before that account can hold its asset.
    /// https://developers.stellar.org/docs/tokens/control-asset-access#authorization-required-0x1
    pub set_required: bool,
    #[arg(long, conflicts_with = "clear_revocable")]
    /// When enabled, an issuer can revoke an existing trustline's authorization, thereby freezing the asset held by an account.
    /// https://developers.stellar.org/docs/tokens/control-asset-access#authorization-revocable-0x2
    pub set_revocable: bool,
    #[arg(long, conflicts_with = "clear_clawback_enabled")]
    /// Enables the issuing account to take back (burning) all of the asset.
    /// https://developers.stellar.org/docs/tokens/control-asset-access#clawback-enabled-0x8
    pub set_clawback_enabled: bool,
    #[arg(long, conflicts_with = "clear_immutable")]
    /// With this setting, none of the other authorization flags (`AUTH_REQUIRED_FLAG`, `AUTH_REVOCABLE_FLAG`) can be set, and the issuing account can't be merged.
    /// https://developers.stellar.org/docs/tokens/control-asset-access#authorization-immutable-0x4
    pub set_immutable: bool,
    #[arg(long)]
    pub clear_required: bool,
    #[arg(long)]
    pub clear_revocable: bool,
    #[arg(long)]
    pub clear_immutable: bool,
    #[arg(long)]
    pub clear_clawback_enabled: bool,
}

impl From<&Args> for xdr::OperationBody {
    fn from(cmd: &Args) -> Self {
        let mut set_flags = None;
        let mut set_flag = |flag: xdr::AccountFlags| {
            *set_flags.get_or_insert(0) |= flag as u32;
        };

        if cmd.set_required {
            set_flag(xdr::AccountFlags::RequiredFlag);
        };
        if cmd.set_revocable {
            set_flag(xdr::AccountFlags::RevocableFlag);
        };
        if cmd.set_immutable {
            set_flag(xdr::AccountFlags::ImmutableFlag);
        };
        if cmd.set_clawback_enabled {
            set_flag(xdr::AccountFlags::ClawbackEnabledFlag);
        };

        let mut clear_flags = None;
        let mut clear_flag = |flag: xdr::AccountFlags| {
            *clear_flags.get_or_insert(0) |= flag as u32;
        };
        if cmd.clear_required {
            clear_flag(xdr::AccountFlags::RequiredFlag);
        };
        if cmd.clear_revocable {
            clear_flag(xdr::AccountFlags::RevocableFlag);
        };
        if cmd.clear_immutable {
            clear_flag(xdr::AccountFlags::ImmutableFlag);
        };
        if cmd.clear_clawback_enabled {
            clear_flag(xdr::AccountFlags::ClawbackEnabledFlag);
        };

        let signer = if let (Some(key), Some(signer_weight)) =
            (cmd.signer.clone(), cmd.signer_weight.as_ref())
        {
            Some(xdr::Signer {
                key,
                weight: u32::from(*signer_weight),
            })
        } else {
            None
        };
        xdr::OperationBody::SetOptions(xdr::SetOptionsOp {
            inflation_dest: cmd.inflation_dest.clone().map(Into::into),
            clear_flags,
            set_flags,
            master_weight: cmd.master_weight.map(Into::into),
            low_threshold: cmd.low_threshold.map(Into::into),
            med_threshold: cmd.med_threshold.map(Into::into),
            high_threshold: cmd.high_threshold.map(Into::into),
            home_domain: cmd.home_domain.clone().map(Into::into),
            signer,
        })
    }
}
