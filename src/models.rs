//! Common domain types: transactions and account state.

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// All transaction kinds supported by the spec.
///
/// We derive `PartialEq`/`Eq` so we can compare directly
/// (e.g. `kind == TxType::Deposit`).
#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TxType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

/// A single input row as parsed from the CSV.
///
/// *The `amount` field is optional* – it is present **only**
/// for `deposit` and `withdrawal` rows.
#[derive(Debug, Deserialize)]
pub struct Transaction {
    /// Operation type (deposit, withdrawal, …).
    #[serde(rename = "type")]
    pub kind: TxType,
    /// Client identifier (0-65 535).
    pub client: u16,
    /// Unique transaction id (0-4 294 967 295).
    pub tx: u32,
    /// Monetary amount (only for deposit / withdrawal).
    #[serde(default)]
    pub amount: Option<Decimal>,
}

/// Runtime state of a client account.
///
/// * `available` – funds free to use or withdraw  
/// * `held`      – funds locked in ongoing disputes  
/// * `locked`    – `true` after a successful chargeback
#[derive(Default, Debug)]
pub struct Account {
    pub available: Decimal,
    pub held: Decimal,
    pub locked: bool,
}

impl Account {
    /// Convenience - total = available + held.
    pub fn total(&self) -> Decimal {
        self.available + self.held
    }
}

/// Helper struct used only for CSV output (serde serialise).
#[derive(Serialize)]
pub struct AccountRow {
    pub client: u16,
    pub available: String,
    pub held: String,
    pub total: String,
    pub locked: bool,
}

impl From<(&u16, &Account)> for AccountRow {
    fn from((client, acc): (&u16, &Account)) -> Self {
        // Round to 4 dp as required by the Kraken spec.
        let fmt = |d: Decimal| format!("{:.4}", d.round_dp(4));
        Self {
            client: *client,
            available: fmt(acc.available),
            held: fmt(acc.held),
            total: fmt(acc.total()),
            locked: acc.locked,
        }
    }
}
