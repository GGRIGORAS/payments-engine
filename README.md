# Payments Engine — Take-Home Exercise

A small Rust CLI that streams a CSV list of transactions and prints the closing
balance for every client, exactly as required  **toy payments
engine** brief.

| Command                                           | Purpose                                                         |
| ------------------------------------------------- | --------------------------------------------------------------- |
| `cargo build --release`                           | Build an optimized binary in `target/release/payments_engine`. |
| `cargo run -- transactions.csv > accounts.csv`    | Run the engine on the sample input and write results to `stdout`. |

---

## Design notes & assumptions

* **Fixed-point math** — uses `rust_decimal`; all amounts are rounded to **4 dp**.  
* **Streaming** — the CSV is processed row-by-row; memory grows only with **open** deposits.  
* **Idempotency** — a repeated `tx` id is ignored after the first valid occurrence.  
* **Error handling** — malformed or out-of-sequence rows are skipped (logged via `anyhow`).  
* **Freeze rule** — a successful `chargeback` locks the account; further ops are ignored.  

---

## Complexity

| Operation             | Time | Space          | Notes                                                                 |
| --------------------- | ---- | -------------- | --------------------------------------------------------------------- |
| Process N rows        | O(N) | —              | Single forward pass.                                                  |
| Hash-map look-ups     | O(1) avg | —          | `accounts`, `deposits` — amortized constant-time.                     |
| Total memory          | —    | O(C + D)       | `C` = #clients, `D` = open deposits. `D ≤ N` and shrinks on resolve/chargeback. |

Empirical throughput on a MacBook M1 (release build) ≈ **0.75 M rows/s**;
bottleneck is CSV parsing, not map access.

---

## Project layout

```text
.
├─ Cargo.toml
├─ README.md
├─ sample-data/
│  └─ transactions.csv   # 5-line sample from the spec
├─ src/
│  ├─ main.rs            # CLI wrapper
│  ├─ engine.rs          # core logic (+ unit tests)
│  ├─ models.rs          # structs & enums
│  └─ errors.rs          # anyhow::Result alias
└─ accounts.csv          # output example (git-ignored in CI)
