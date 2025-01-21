pub const ACCOUNT_MERGE:&str = "Transfers the XLM balance of an account to another account and removes the source account from the ledger";
pub const BUMP_SEQUENCE: &str = "Bumps forward the sequence number of the source account to the given sequence number, invalidating any transaction with a smaller sequence number";
pub const CHANGE_TRUST: &str = r"Creates, updates, or deletes a trustline
Learn more about trustlines
https://developers.stellar.org/docs/learn/fundamentals/stellar-data-structures/accounts#trustlines";

pub const CREATE_ACCOUNT: &str =
    "Creates and funds a new account with the specified starting balance";
pub const MANAGE_DATA: &str = r"Sets, modifies, or deletes a data entry (name/value pair) that is attached to an account
Learn more about entries and subentries:
https://developers.stellar.org/docs/learn/fundamentals/stellar-data-structures/accounts#subentries";
pub const PAYMENT: &str = "Sends an amount in a specific asset to a destination account";
pub const SET_OPTIONS: &str = r"Set option for an account such as flags, inflation destination, signers, home domain, and master key weight
Learn more about flags:
https://developers.stellar.org/docs/learn/glossary#flags
Learn more about the home domain:
https://github.com/stellar/stellar-protocol/blob/master/ecosystem/sep-0001.md
Learn more about signers operations and key weight:
https://developers.stellar.org/docs/learn/encyclopedia/security/signatures-multisig#multisig";
pub const SET_TRUSTLINE_FLAGS: &str = r"Allows issuing account to configure authorization and trustline flags to an asset
The Asset parameter is of the `TrustLineAsset` type. If you are modifying a trustline to a regular asset (i.e. one in a Code:Issuer format), this is equivalent to the Asset type.
If you are modifying a trustline to a pool share, however, this is composed of the liquidity pool's unique ID.
Learn more about flags:
https://developers.stellar.org/docs/learn/glossary#flags";
