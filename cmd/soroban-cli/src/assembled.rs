use sha2::{Digest, Sha256};
use stellar_xdr::curr::{
    self as xdr, ExtensionPoint, Hash, InvokeHostFunctionOp, LedgerFootprint, Limits, Memo,
    Operation, OperationBody, Preconditions, ReadXdr, RestoreFootprintOp,
    SorobanAuthorizationEntry, SorobanAuthorizedFunction, SorobanResources, SorobanTransactionData,
    Transaction, TransactionEnvelope, TransactionExt, TransactionSignaturePayload,
    TransactionSignaturePayloadTaggedTransaction, TransactionV1Envelope, VecM, WriteXdr,
};

use soroban_rpc::{Error, RestorePreamble, SimulateTransactionResponse};

use soroban_rpc::{LogEvents, LogResources};

pub(crate) const DEFAULT_TRANSACTION_FEES: u32 = 100;

pub async fn simulate_and_assemble_transaction(
    client: &soroban_rpc::Client,
    tx: &Transaction,
) -> Result<Assembled, Error> {
    let sim_res = client
        .simulate_transaction_envelope(&TransactionEnvelope::Tx(TransactionV1Envelope {
            tx: tx.clone(),
            signatures: VecM::default(),
        }))
        .await?;
    tracing::trace!("{sim_res:#?}");
    if let Some(e) = &sim_res.error {
        crate::log::event::all(&sim_res.events()?);
        Err(Error::TransactionSimulationFailed(e.clone()))
    } else {
        Ok(Assembled::new(tx, sim_res)?)
    }
}

pub struct Assembled {
    pub(crate) txn: Transaction,
    pub(crate) sim_res: SimulateTransactionResponse,
}

/// Represents an assembled transaction ready to be signed and submitted to the network.
impl Assembled {
    ///
    /// Creates a new `Assembled` transaction.
    ///
    /// # Arguments
    ///
    /// * `txn` - The original transaction.
    /// * `client` - The client used for simulation and submission.
    ///
    /// # Errors
    ///
    /// Returns an error if simulation fails or if assembling the transaction fails.
    pub fn new(txn: &Transaction, sim_res: SimulateTransactionResponse) -> Result<Self, Error> {
        let txn = assemble(txn, &sim_res)?;
        Ok(Self { txn, sim_res })
    }

    ///
    /// Calculates the hash of the assembled transaction.
    ///
    /// # Arguments
    ///
    /// * `network_passphrase` - The network passphrase.
    ///
    /// # Errors
    ///
    /// Returns an error if generating the hash fails.
    pub fn hash(&self, network_passphrase: &str) -> Result<[u8; 32], xdr::Error> {
        let signature_payload = TransactionSignaturePayload {
            network_id: Hash(Sha256::digest(network_passphrase).into()),
            tagged_transaction: TransactionSignaturePayloadTaggedTransaction::Tx(self.txn.clone()),
        };
        Ok(Sha256::digest(signature_payload.to_xdr(Limits::none())?).into())
    }

    ///  Create a transaction for restoring any data in the `restore_preamble` field of the `SimulateTransactionResponse`.
    ///
    /// # Errors
    pub fn restore_txn(&self) -> Result<Option<Transaction>, Error> {
        if let Some(restore_preamble) = &self.sim_res.restore_preamble {
            restore(self.transaction(), restore_preamble).map(Option::Some)
        } else {
            Ok(None)
        }
    }

    /// Returns a reference to the original transaction.
    #[must_use]
    pub fn transaction(&self) -> &Transaction {
        &self.txn
    }

    /// Returns a reference to the simulation response.
    #[must_use]
    pub fn sim_response(&self) -> &SimulateTransactionResponse {
        &self.sim_res
    }

    #[must_use]
    pub fn bump_seq_num(mut self) -> Self {
        self.txn.seq_num.0 += 1;
        self
    }

    ///
    /// # Errors
    #[must_use]
    pub fn auth_entries(&self) -> VecM<SorobanAuthorizationEntry> {
        self.txn
            .operations
            .first()
            .and_then(|op| match op.body {
                OperationBody::InvokeHostFunction(ref body) => (matches!(
                    body.auth.first().map(|x| &x.root_invocation.function),
                    Some(&SorobanAuthorizedFunction::ContractFn(_))
                ))
                .then_some(body.auth.clone()),
                _ => None,
            })
            .unwrap_or_default()
    }

    ///
    /// # Errors
    pub fn log(
        &self,
        log_events: Option<LogEvents>,
        log_resources: Option<LogResources>,
    ) -> Result<(), Error> {
        if let TransactionExt::V1(SorobanTransactionData {
            resources: resources @ SorobanResources { footprint, .. },
            ..
        }) = &self.txn.ext
        {
            if let Some(log) = log_resources {
                log(resources);
            }
            if let Some(log) = log_events {
                log(footprint, &[self.auth_entries()], &self.sim_res.events()?);
            };
        }
        Ok(())
    }

    #[must_use]
    pub fn requires_auth(&self) -> bool {
        requires_auth(&self.txn).is_some()
    }

    #[must_use]
    pub fn is_view(&self) -> bool {
        let TransactionExt::V1(SorobanTransactionData {
            resources:
                SorobanResources {
                    footprint: LedgerFootprint { read_write, .. },
                    ..
                },
            ..
        }) = &self.txn.ext
        else {
            return false;
        };
        read_write.is_empty()
    }

    #[must_use]
    pub fn set_max_instructions(mut self, instructions: u32) -> Self {
        if let TransactionExt::V1(SorobanTransactionData {
            resources:
                SorobanResources {
                    instructions: ref mut i,
                    ..
                },
            ..
        }) = &mut self.txn.ext
        {
            tracing::trace!("setting max instructions to {instructions} from {i}");
            *i = instructions;
        }
        self
    }
}

// Apply the result of a simulateTransaction onto a transaction envelope, preparing it for
// submission to the network.
///
/// # Errors
fn assemble(
    raw: &Transaction,
    simulation: &SimulateTransactionResponse,
) -> Result<Transaction, Error> {
    let mut tx = raw.clone();

    // Right now simulate.results is one-result-per-function, and assumes there is only one
    // operation in the txn, so we need to enforce that here. I (Paul) think that is a bug
    // in soroban-rpc.simulateTransaction design, and we should fix it there.
    // TODO: We should to better handling so non-soroban txns can be a passthrough here.
    if tx.operations.len() != 1 {
        return Err(Error::UnexpectedOperationCount {
            count: tx.operations.len(),
        });
    }

    let transaction_data = simulation.transaction_data()?;

    let mut op = tx.operations[0].clone();
    if let OperationBody::InvokeHostFunction(ref mut body) = &mut op.body {
        if body.auth.is_empty() {
            if simulation.results.len() != 1 {
                return Err(Error::UnexpectedSimulateTransactionResultSize {
                    length: simulation.results.len(),
                });
            }

            let auths = simulation
                .results
                .iter()
                .map(|r| {
                    VecM::try_from(
                        r.auth
                            .iter()
                            .map(|v| SorobanAuthorizationEntry::from_xdr_base64(v, Limits::none()))
                            .collect::<Result<Vec<_>, _>>()?,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            if !auths.is_empty() {
                body.auth = auths[0].clone();
            }
        }
    }

    // Update transaction fees to meet the minimum resource fees.
    let classic_tx_fee: u64 = DEFAULT_TRANSACTION_FEES.into();

    // Choose larger of existing fee or inclusion + resource fee.
    tx.fee = tx.fee.max(
        u32::try_from(classic_tx_fee + simulation.min_resource_fee)
            .map_err(|_| Error::LargeFee(simulation.min_resource_fee + classic_tx_fee))?,
    );

    tx.operations = vec![op].try_into()?;
    tx.ext = TransactionExt::V1(transaction_data);
    Ok(tx)
}

fn requires_auth(txn: &Transaction) -> Option<xdr::Operation> {
    let [op @ Operation {
        body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp { auth, .. }),
        ..
    }] = txn.operations.as_slice()
    else {
        return None;
    };
    matches!(
        auth.first().map(|x| &x.root_invocation.function),
        Some(&SorobanAuthorizedFunction::ContractFn(_))
    )
    .then(move || op.clone())
}

fn restore(parent: &Transaction, restore: &RestorePreamble) -> Result<Transaction, Error> {
    let transaction_data =
        SorobanTransactionData::from_xdr_base64(&restore.transaction_data, Limits::none())?;
    let fee = u32::try_from(restore.min_resource_fee)
        .map_err(|_| Error::LargeFee(restore.min_resource_fee))?;
    Ok(Transaction {
        source_account: parent.source_account.clone(),
        fee: parent
            .fee
            .checked_add(fee)
            .ok_or(Error::LargeFee(restore.min_resource_fee))?,
        seq_num: parent.seq_num.clone(),
        cond: Preconditions::None,
        memo: Memo::None,
        operations: vec![Operation {
            source_account: None,
            body: OperationBody::RestoreFootprint(RestoreFootprintOp {
                ext: ExtensionPoint::V0,
            }),
        }]
        .try_into()?,
        ext: TransactionExt::V1(transaction_data),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use soroban_rpc::SimulateHostFunctionResultRaw;
    use stellar_strkey::ed25519::PublicKey as Ed25519PublicKey;
    use stellar_xdr::curr::{
        AccountId, ChangeTrustAsset, ChangeTrustOp, ExtensionPoint, Hash, HostFunction,
        InvokeContractArgs, InvokeHostFunctionOp, LedgerFootprint, Memo, MuxedAccount, Operation,
        Preconditions, PublicKey, ScAddress, ScSymbol, ScVal, SequenceNumber,
        SorobanAuthorizedFunction, SorobanAuthorizedInvocation, SorobanResources,
        SorobanTransactionData, Uint256, WriteXdr,
    };

    const SOURCE: &str = "GBZXN7PIRZGNMHGA7MUUUF4GWPY5AYPV6LY4UV2GL6VJGIQRXFDNMADI";

    fn transaction_data() -> SorobanTransactionData {
        SorobanTransactionData {
            resources: SorobanResources {
                footprint: LedgerFootprint {
                    read_only: VecM::default(),
                    read_write: VecM::default(),
                },
                instructions: 0,
                read_bytes: 5,
                write_bytes: 0,
            },
            resource_fee: 0,
            ext: ExtensionPoint::V0,
        }
    }

    fn simulation_response() -> SimulateTransactionResponse {
        let source_bytes = Ed25519PublicKey::from_string(SOURCE).unwrap().0;
        let fn_auth = &SorobanAuthorizationEntry {
            credentials: xdr::SorobanCredentials::Address(xdr::SorobanAddressCredentials {
                address: ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(Uint256(
                    source_bytes,
                )))),
                nonce: 0,
                signature_expiration_ledger: 0,
                signature: ScVal::Void,
            }),
            root_invocation: SorobanAuthorizedInvocation {
                function: SorobanAuthorizedFunction::ContractFn(InvokeContractArgs {
                    contract_address: ScAddress::Contract(Hash([0; 32])),
                    function_name: ScSymbol("fn".try_into().unwrap()),
                    args: VecM::default(),
                }),
                sub_invocations: VecM::default(),
            },
        };

        SimulateTransactionResponse {
            min_resource_fee: 115,
            latest_ledger: 3,
            results: vec![SimulateHostFunctionResultRaw {
                auth: vec![fn_auth.to_xdr_base64(Limits::none()).unwrap()],
                xdr: ScVal::U32(0).to_xdr_base64(Limits::none()).unwrap(),
            }],
            transaction_data: transaction_data().to_xdr_base64(Limits::none()).unwrap(),
            ..Default::default()
        }
    }

    fn single_contract_fn_transaction() -> Transaction {
        let source_bytes = Ed25519PublicKey::from_string(SOURCE).unwrap().0;
        Transaction {
            source_account: MuxedAccount::Ed25519(Uint256(source_bytes)),
            fee: 100,
            seq_num: SequenceNumber(0),
            cond: Preconditions::None,
            memo: Memo::None,
            operations: vec![Operation {
                source_account: None,
                body: OperationBody::InvokeHostFunction(InvokeHostFunctionOp {
                    host_function: HostFunction::InvokeContract(InvokeContractArgs {
                        contract_address: ScAddress::Contract(Hash([0x0; 32])),
                        function_name: ScSymbol::default(),
                        args: VecM::default(),
                    }),
                    auth: VecM::default(),
                }),
            }]
            .try_into()
            .unwrap(),
            ext: TransactionExt::V0,
        }
    }

    #[test]
    fn test_assemble_transaction_updates_tx_data_from_simulation_response() {
        let sim = simulation_response();
        let txn = single_contract_fn_transaction();
        let Ok(result) = assemble(&txn, &sim) else {
            panic!("assemble failed");
        };

        // validate it auto updated the tx fees from sim response fees
        // since it was greater than tx.fee
        assert_eq!(215, result.fee);

        // validate it updated sorobantransactiondata block in the tx ext
        assert_eq!(TransactionExt::V1(transaction_data()), result.ext);
    }

    #[test]
    fn test_assemble_transaction_adds_the_auth_to_the_host_function() {
        let sim = simulation_response();
        let txn = single_contract_fn_transaction();
        let Ok(result) = assemble(&txn, &sim) else {
            panic!("assemble failed");
        };

        assert_eq!(1, result.operations.len());
        let OperationBody::InvokeHostFunction(ref op) = result.operations[0].body else {
            panic!("unexpected operation type: {:#?}", result.operations[0]);
        };

        assert_eq!(1, op.auth.len());
        let auth = &op.auth[0];

        let xdr::SorobanAuthorizedFunction::ContractFn(xdr::InvokeContractArgs {
            ref function_name,
            ..
        }) = auth.root_invocation.function
        else {
            panic!("unexpected function type");
        };
        assert_eq!("fn".to_string(), format!("{}", function_name.0));

        let xdr::SorobanCredentials::Address(xdr::SorobanAddressCredentials {
            address:
                xdr::ScAddress::Account(xdr::AccountId(xdr::PublicKey::PublicKeyTypeEd25519(address))),
            ..
        }) = &auth.credentials
        else {
            panic!("unexpected credentials type");
        };
        assert_eq!(
            SOURCE.to_string(),
            stellar_strkey::ed25519::PublicKey(address.0).to_string()
        );
    }

    #[test]
    fn test_assemble_transaction_errors_for_non_invokehostfn_ops() {
        let source_bytes = Ed25519PublicKey::from_string(SOURCE).unwrap().0;
        let txn = Transaction {
            source_account: MuxedAccount::Ed25519(Uint256(source_bytes)),
            fee: 100,
            seq_num: SequenceNumber(0),
            cond: Preconditions::None,
            memo: Memo::None,
            operations: vec![Operation {
                source_account: None,
                body: OperationBody::ChangeTrust(ChangeTrustOp {
                    line: ChangeTrustAsset::Native,
                    limit: 0,
                }),
            }]
            .try_into()
            .unwrap(),
            ext: TransactionExt::V0,
        };

        let result = assemble(
            &txn,
            &SimulateTransactionResponse {
                min_resource_fee: 115,
                transaction_data: transaction_data().to_xdr_base64(Limits::none()).unwrap(),
                latest_ledger: 3,
                ..Default::default()
            },
        );

        match result {
            Ok(_) => {}
            Err(e) => panic!("expected assembled operation, got: {e:#?}"),
        }
    }

    #[test]
    fn test_assemble_transaction_errors_for_errors_for_mismatched_simulation() {
        let txn = single_contract_fn_transaction();

        let result = assemble(
            &txn,
            &SimulateTransactionResponse {
                min_resource_fee: 115,
                transaction_data: transaction_data().to_xdr_base64(Limits::none()).unwrap(),
                latest_ledger: 3,
                ..Default::default()
            },
        );

        match result {
            Err(Error::UnexpectedSimulateTransactionResultSize { length }) => {
                assert_eq!(0, length);
            }
            r => panic!("expected UnexpectedSimulateTransactionResultSize error, got: {r:#?}"),
        }
    }

    #[test]
    fn test_assemble_transaction_overflow_behavior() {
        //
        // Test two separate cases:
        //
        //  1. Given a near-max (u32::MAX - 100) resource fee make sure the tx
        //     fee does not overflow after adding the base inclusion fee (100).
        //  2. Given a large resource fee that WILL exceed u32::MAX with the
        //     base inclusion fee, ensure the overflow is caught with an error
        //     rather than silently ignored.
        let txn = single_contract_fn_transaction();
        let mut response = simulation_response();

        // sanity check so these can be adjusted if the above helper changes
        assert_eq!(txn.fee, 100, "modified txn.fee: update the math below");

        // 1: wiggle room math overflows but result fits
        response.min_resource_fee = (u32::MAX - 100).into();

        match assemble(&txn, &response) {
            Ok(asstxn) => {
                let expected = u32::MAX;
                assert_eq!(asstxn.fee, expected);
            }
            r => panic!("expected success, got: {r:#?}"),
        }

        // 2: combo overflows, should throw
        response.min_resource_fee = (u32::MAX - 99).into();

        match assemble(&txn, &response) {
            Err(Error::LargeFee(fee)) => {
                let expected = u64::from(u32::MAX) + 1;
                assert_eq!(expected, fee, "expected {expected} != {fee} actual");
            }
            r => panic!("expected LargeFee error, got: {r:#?}"),
        }
    }
}
