# Escrow Batch Funding: `fund_batch` (Canonical Entrypoint)

## Overview

`fund_batch` is the **canonical** batch funding entrypoint for the karis-ky escrow contract. It is the single public interface for multi-investor funding in a single atomic transaction.

**Decision:** This document establishes `fund_batch` as canonical to prevent duplication and implementation drift. All batch funding requests must use `fund_batch`.

---

## ⚠ Critical: Maturity Checks Use Ledger Timestamp

**All settlement operations (`settle()` and `partial_settle()`) use the **validator-observed ledger timestamp** 
(`env.ledger().timestamp()`) to enforce maturity gates, NOT wall-clock time.** This is essential for 
off-chain tooling and operator dashboards that monitor settlement eligibility. See 
[Escrow Lifecycle — Maturity Gate](escrow-lifecycle.md#ledger-timestamp-basis-critical-for-operators) 
for complete details, boundary examples, and implications for operators integrating external systems.

---

## Quick Comparison: `fund` vs. `fund_with_commitment`

### Canonical Entrypoint
- **Name:** `fund_batch`
- **Signature:** `pub fn fund_batch(env: Env, entries: Vec<(Address, i128)>) -> InvoiceEscrow`
- **Status:** Stable, long-term support

### Alternative Names (Future Deprecation)
If a naming variant (e.g., `batch_fund`) is ever added as an alias or alternative implementation:

1. It **must** be marked with the `#[deprecated]` attribute in Rust:
   ```rust
   #[deprecated(
       since = "<version>",
       note = "use `fund_batch` instead; this is deprecated for consolidation"
   )]
   pub fn batch_fund(env: Env, entries: Vec<(Address, i128)>) -> InvoiceEscrow {
       // Forward to canonical implementation
       Self::fund_batch(env, entries)
   }
   ```

2. The deprecation note **must** be visible to SDK codegen and documentation tools (`cargo doc`).

3. TypeScript client wrappers **must not** expose the deprecated name. Only `fundBatch` should be available, calling the canonical `fund_batch` entrypoint.

---

## Semantic Specification

### Function Signature
```rust
pub fn fund_batch(env: Env, entries: Vec<(Address, i128)>) -> InvoiceEscrow
```

### Parameters
- **`entries`:** Vector of `(Address, i128)` tuples, where:
  - First element: investor address
  - Second element: funding amount (in base units of the funding token)
  - Must be non-empty: `len() > 0` → error `EscrowError::FundingBatchEmpty`
  - Must not exceed capacity: `len() <= MAX_FUND_BATCH` (50) → error `EscrowError::FundingBatchTooLarge`

### Processing Order
Entries are processed **sequentially**, not in parallel:
1. For each `(investor, amount)` tuple (order preserved):
   - Call `investor.require_auth()` (per-investor authorization)
   - Validate and record the contribution
   - Update escrow state
   - Emit per-entry events

### Return Value
The updated [`InvoiceEscrow`] struct reflecting all entries applied.

### Events Emitted
- **Per entry:** One `EscrowFunded` event (identical to single `fund()` semantics)
- **Per entry:** One `FundReceived` event with yield/commitment details
- **No batch-level summary event** (each entry is independently auditable)

### Invariants Enforced

All invariants from [`LiquifactEscrow::fund`] apply to each entry:

| Invariant | Error on Violation | Notes |
|-----------|-------------------|-------|
| Amount > 0 | `EscrowAmountNotPositive` | Per entry |
| Amount >= min_contribution floor | `FundingBelowMinContribution` | If floor is set |
| Escrow in open status | `EscrowNotOpenForFunding` | Per entry |
| Investor allowlisted (if gate active) | `InvestorNotAllowlisted` | Per entry |
| Investor contribution + amount ≤ max_per_investor | `InvestorContributionExceedsCap` | Per entry |
| New investor would not exceed cap | `UniqueInvestorCapReached` | Per entry |
| No arithmetic overflow | `InvestorContributionOverflow` / `FundedAmountOverflow` | Per entry |
| Legal hold not active | `LegalHoldBlocksFunding` | Checked once; blocks entire batch |

### Status Transitions

If any entry causes the escrow to transition from **open** (`status 0`) to **funded** (`status 1`):

1. **Transition point:** Recorded exactly **once**, the first time `funded_amount >= funding_target`
2. **Snapshot:** [`DataKey::FundingCloseSnapshot`] is written atomically with:
   - Total principal at transition
   - Funding target
   - Ledger timestamp
   - Ledger sequence number
3. **Remaining entries:** Continue to be processed after the transition (over-funding is allowed)
4. **Future entries:** Cannot regress the status back to open

### Atomicity & Failure Semantics

**Soroban transaction atomicity guarantee:**
- If **any** entry fails validation, the **entire batch is rolled back** without persisting any state changes.
- No "partial funding" state is ever persisted.
- Exception: If validation fails after some entries have already been applied, the error is returned and the transaction atomically reverts all changes from this call.

---

## Example Usage

### Rust (Direct Contract Call)
```rust
let entries = vec![
    (investor_a_address, 30_000i128),
    (investor_b_address, 55_000i128),
    (investor_c_address, 10_000i128),
];

let result = contract.fund_batch(entries);
// All three investors funded (or entire batch reverted on first error)
```

### TypeScript (SDK Client)
```typescript
const entries: Array<[string, string]> = [
    ["GINVESTOR1...", "30000000000"],  // 3,000.0 tokens (7 decimals)
    ["GINVESTOR2...", "55000000000"],  // 5,500.0 tokens
    ["GINVESTOR3...", "10000000000"],  // 1,000.0 tokens
];

const updatedEscrow = await client.fundBatch(entries);
console.log(`Escrow status: ${updatedEscrow.status}`); // 0 or 1
```

---

## Comparison with Single Funding

### Equivalence Guarantee
A batch call with entries `[(A, x), (B, y), (C, z)]` **must** produce **identical** results to:
```
fund(A, x) → fund(B, y) → fund(C, z)
```

Specifically:
- Same per-entry events in the same order
- Same escrow state transitions
- Same funding_close snapshot timestamp (if triggered)
- Same error handling (first error aborts the batch)

### Transaction Cost Advantage
Batch funding reduces:
- Number of Soroban transactions from N (sequential) to 1 (batch)
- Ledger inclusion latency
- RPC round-trips

---

## Error Codes

| Code | Error | Trigger | Recovery |
|------|-------|---------|----------|
| `EscrowError::FundingBatchEmpty` | Batch size is 0 | `entries.is_empty()` | Provide at least one entry |
| `EscrowError::FundingBatchTooLarge` | Batch size > 50 | `entries.len() > MAX_FUND_BATCH` | Split into multiple batches |
| (Any `fund()` error) | Per-entry validation failed | See [`LiquifactEscrow::fund`] | Fix the failing entry and retry entire batch |

---

## Migration Notes

### From Multiple Sequential `fund()` Calls
If your off-chain issuance pipeline currently calls `fund()` N times in a loop:

**Before:**
```typescript
for (const [investor, amount] of investors) {
  await client.fund(investor, amount);  // N transactions
}
```

**After:**
```typescript
const entries = investors.map(([investor, amount]) => [investor, amount]);
await client.fundBatch(entries);  // 1 transaction
```

### No Breaking Changes
- Existing `fund()` and `fund_with_commitment()` entrypoints remain unchanged.
- `fund_batch` is a **new** capability, not a replacement; existing integrations are unaffected.

---

## See Also

- [Escrow Lifecycle](escrow-lifecycle.md) — full state machine and transitions
- [Escrow Init Parameters](escrow-init-parameters.md) — configuring caps and constraints
- [Escrow Pro-Rata Payout](escrow-pro-rata.md) — how batch contributions affect share calculation
- Rust contract: `escrow/src/lib.rs::LiquifactEscrow::fund_batch`
- TypeScript SDK: `sdk-ts/src/client.ts::EscrowClient::fundBatch`
