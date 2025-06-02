//! Stream-oriented payments engine: feed one CSV row at a time, then
//! read `engine.accounts` to produce the final per-client report.

use crate::errors::Result;
use crate::models::{Account, Transaction, TxType};
use rust_decimal::Decimal;
use std::collections::HashMap;

/// Record of an original *deposit* so a later dispute / resolve / chargeback
/// can reference the amount and the owning client.
#[derive(Debug)]
struct StoredTx {
    client: u16,
    amount: Decimal,
    under_dispute: bool,
}

/// In-memory engine.
///
/// ```rust
/// let mut eng = Engine::new();
/// for tx in csv_rows {
///     eng.process(tx)?;
/// }
/// for (id, acc) in &eng.accounts { /* emit CSV */ }
/// ```
pub struct Engine {
    /// Map of client-id ➜ current account state.
    pub accounts: HashMap<u16, Account>,
    /// Map of deposit tx-id ➜ stored data (needed for disputes).
    deposits: HashMap<u32, StoredTx>,
}

impl Engine {
    /// Create a brand-new, empty engine.
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            deposits: HashMap::new(),
        }
    }

    /// Apply a single transaction to the internal state.
    ///
    /// * Invalid rows are ignored *without* creating empty accounts.  
    /// * After a successful **chargeback** the affected account is frozen;  
    ///   further operations on it are silently dropped.
    pub fn process(&mut self, tx: Transaction) -> Result<()> {
        match tx.kind {
            // ----------------------------------------------------------- deposit
            TxType::Deposit => {
                let amount = match tx.amount {
                    Some(a) if a > Decimal::ZERO => a,
                    _ => return Ok(()), // reject zero / negative
                };
                if self.deposits.contains_key(&tx.tx) {
                    return Ok(());
                }
                let acc = self.accounts.entry(tx.client).or_default();
                if acc.locked {
                    return Ok(());
                }

                acc.available += amount;
                self.deposits.insert(
                    tx.tx,
                    StoredTx {
                        client: tx.client,
                        amount,
                        under_dispute: false,
                    },
                );
            }

            // --------------------------------------------------------- withdrawal
            TxType::Withdrawal => {
                let amount = match tx.amount {
                    Some(a) if a > Decimal::ZERO => a,
                    _ => return Ok(()),
                };
                if let Some(acc) = self.accounts.get_mut(&tx.client) {
                    if !acc.locked && acc.available >= amount {
                        acc.available -= amount;
                    }
                }
            }

            // --------------------------- dispute / resolve / chargeback ----------
            TxType::Dispute | TxType::Resolve | TxType::Chargeback => {
                if let (Some(dep), Some(acc)) = (
                    self.deposits.get_mut(&tx.tx),
                    self.accounts.get_mut(&tx.client),
                ) {
                    if dep.client != tx.client || acc.locked {
                        return Ok(());
                    }

                    match tx.kind {
                        TxType::Dispute if !dep.under_dispute => {
                            if let Some(new_avail) = acc.available.checked_sub(dep.amount) {
                                acc.available = new_avail;
                                acc.held += dep.amount;
                                dep.under_dispute = true;
                            }
                        }
                        TxType::Resolve if dep.under_dispute => {
                            acc.held -= dep.amount;
                            acc.available += dep.amount;
                            dep.under_dispute = false;
                        }
                        TxType::Chargeback if dep.under_dispute => {
                            acc.held -= dep.amount;
                            acc.locked = true;
                            dep.under_dispute = false;
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    fn tx(kind: TxType, client: u16, id: u32, amt: Option<Decimal>) -> Transaction {
        Transaction {
            kind,
            client,
            tx: id,
            amount: amt,
        }
    }

    #[test]
    fn deposit_then_withdraw() {
        let mut eng = Engine::new();
        eng.process(tx(TxType::Deposit, 1, 1, Some(dec!(2.0))))
            .unwrap();
        eng.process(tx(TxType::Withdrawal, 1, 2, Some(dec!(1.5))))
            .unwrap();

        let acc = &eng.accounts[&1];
        assert_eq!(acc.available, dec!(0.5));
        assert_eq!(acc.held, dec!(0));
        assert!(!acc.locked);
    }

    #[test]
    fn dispute_then_chargeback_locks_account() {
        let mut eng = Engine::new();
        eng.process(tx(TxType::Deposit, 1, 1, Some(dec!(3.0))))
            .unwrap();
        eng.process(tx(TxType::Dispute, 1, 1, None)).unwrap();
        eng.process(tx(TxType::Chargeback, 1, 1, None)).unwrap();
        eng.process(tx(TxType::Deposit, 1, 2, Some(dec!(1.0))))
            .unwrap(); // ignored

        let acc = &eng.accounts[&1];
        assert_eq!(acc.available, dec!(0));
        assert_eq!(acc.held, dec!(0));
        assert!(acc.locked);
    }

    #[test]
    fn duplicate_dispute_is_ignored() {
        let mut eng = Engine::new();
        eng.process(tx(TxType::Deposit, 1, 10, Some(dec!(5.0))))
            .unwrap();
        eng.process(tx(TxType::Dispute, 1, 10, None)).unwrap();
        eng.process(tx(TxType::Dispute, 1, 10, None)).unwrap(); // duplicate

        let acc = &eng.accounts[&1];
        assert_eq!(acc.available, dec!(0));
        assert_eq!(acc.held, dec!(5.0));
    }

    #[test]
    fn negative_or_zero_amount_is_rejected() {
        let mut eng = Engine::new();
        eng.process(tx(TxType::Deposit, 1, 20, Some(dec!(0))))
            .unwrap();
        eng.process(tx(TxType::Withdrawal, 1, 21, Some(dec!(-1.0))))
            .unwrap();

        assert!(eng.accounts.is_empty());
    }
}
