//! Core payments engine: processes transactions in a streaming fashion.
//!
//! ### Example
//! ```rust,no_run
//! use payments_engine::{Engine, models::{TxType, Transaction}};
//! use csv::ReaderBuilder;
//!
//! // create engine
//! let mut eng = Engine::new();
//!
//! // sample CSV string
//! let csv = "type,client,tx,amount\ndeposit,1,1,1.0\nwithdrawal,1,2,0.5\n";
//!
//! // feed rows
//! let mut rdr = ReaderBuilder::new().trim(csv::Trim::All).from_reader(csv.as_bytes());
//! for row in rdr.deserialize::<Transaction>() {
//!     eng.process(row.unwrap()).unwrap();
//! }
//!
//! // inspect results
//! let acc = &eng.accounts[&1];
//! assert_eq!(acc.available, rust_decimal_macros::dec!(0.5));
//! ```

use crate::errors::Result;
use crate::models::{Account, Transaction, TxType};
use rust_decimal::Decimal;
use std::collections::HashMap;

/// Internal record kept for every *deposit* so later dispute/resolve/chargeback
/// can reference the original amount & client.
#[derive(Debug)]
struct StoredTx {
    client: u16,
    amount: Decimal,
    under_dispute: bool,
}

/// Streaming payments engine. Feed rows via [`Engine::process`] then read
/// `engine.accounts` to generate the final report.
pub struct Engine {
    pub accounts: HashMap<u16, Account>,
    deposits: HashMap<u32, StoredTx>,
}

impl Engine {
    /// Create a new empty engine.
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            deposits: HashMap::new(),
        }
    }

    /// Apply one transaction to the internal state.
    pub fn process(&mut self, tx: Transaction) -> Result<()> {
        // guard: negative or zero amounts are invalid
        if matches!(tx.kind, TxType::Deposit | TxType::Withdrawal) {
            if let Some(a) = tx.amount {
                if a <= Decimal::ZERO {
                    return Ok(());
                }
            }
        }

        // create account on first valid activity
        let acc = self.accounts.entry(tx.client).or_default();

        // ignore any operation on a locked account
        if acc.locked {
            return Ok(());
        }

        match tx.kind {
            TxType::Deposit => {
                let amount = tx.amount.unwrap();
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
            TxType::Withdrawal => {
                let amount = tx.amount.unwrap();
                if acc.available >= amount {
                    acc.available -= amount;
                }
            }
            TxType::Dispute => {
                if let Some(dep) = self.deposits.get_mut(&tx.tx) {
                    if !dep.under_dispute && dep.client == tx.client {
                        dep.under_dispute = true;
                        acc.available -= dep.amount;
                        acc.held += dep.amount;
                    }
                }
            }
            TxType::Resolve => {
                if let Some(dep) = self.deposits.get_mut(&tx.tx) {
                    if dep.under_dispute && dep.client == tx.client {
                        dep.under_dispute = false;
                        acc.available += dep.amount;
                        acc.held -= dep.amount;
                    }
                }
            }
            TxType::Chargeback => {
                if let Some(dep) = self.deposits.get_mut(&tx.tx) {
                    if dep.under_dispute && dep.client == tx.client {
                        dep.under_dispute = false;
                        acc.held -= dep.amount;
                        acc.locked = true;
                    }
                }
            }
        }
        Ok(())
    }
}
