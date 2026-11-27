# Maturity Enforcement — Ledger Timestamp Verification

**Date:** 2025-01-xx  
**Scope:** Confirm that escrow `settle()` and `partial_settle()` enforce maturity against `env.ledger().timestamp()` exclusively, and document this basis for operators and off-chain tooling.

---

## Acceptance Criteria Verification

### ✅ Criterion 1: Settle at maturity - 1 fails with 122; at maturity succeeds

**Status:** VERIFIED

**Test Evidence:**
- File: `/workspaces/KARIS-KY/escrow/src/tests/settlement.rs`
- Test 1: `settle_one_second_before_maturity_traps_and_preserves_state()` (line ~724)
  - Sets `maturity = 20_000`
  - Calls `settle()` at `ledger.timestamp = maturity - 1 = 19_999`
  - **Expected:** Panics with error code 122 (`MaturityNotReached`)
  - **Result:** ✅ Asserts panic occurred, status remains 1, snapshot unchanged
  
- Test 2: `settle_at_maturity_succeeds()` (line ~780)
  - Sets `maturity = 20_000`
  - Calls `settle()` at `ledger.timestamp = maturity = 20_000`
  - **Expected:** Success, status becomes 2 (settled)
  - **Result:** ✅ Asserts `settled.status == 2` and `settled.maturity == maturity`

**Code Reference:**
```rust
// escrow/src/settlement.rs, settle_impl() function
let now = env.ledger().timestamp();
if escrow.maturity > 0 {
    if now < escrow.maturity {
        // ... emit diagnostic with error code 122
        fail(&env, EscrowError::MaturityNotReached);  // ← Code 122 on boundary
    }
}
```

**Boundary Behavior Confirmed:**
- Timestamp `< maturity`: Fails with Code 122 ✗
- Timestamp `>= maturity`: Succeeds ✓
- Maturity boundary is **inclusive** (at or after maturity = permitted)

---

### ✅ Criterion 2: Documentation states the ledger-timestamp basis

**Status:** VERIFIED AND ENHANCED

#### Document 1: `/workspaces/KARIS-KY/docs/escrow-lifecycle.md`

**Previous State:**
```markdown
## Maturity gate

When `maturity > 0`:
- `settle()` requires `env.ledger().timestamp() >= escrow.maturity`
```

**Updated State (Enhanced):**
Added comprehensive section: **"Ledger-Timestamp Basis (Critical for Operators)"**

New content includes:
- Explicit statement: "All maturity checks are strictly against the validator-observed ledger timestamp (`env.ledger().timestamp()`), not wall-clock time"
- Code snippet showing exact check in `settle_impl`
- Boundary behavior table (inclusive boundary at maturity timestamp)
- References to boundary-confirming tests
- **Implications for Operators:**
  1. Off-chain tooling must use the same ledger timestamp
  2. When comparing to external systems, use Stellar ledger timestamp (not UTC wall-clock)
  3. Clock skew across systems can cause confusion; always use contract's `env.ledger().timestamp()`
  4. Maturity is immutable after initialization
- **`partial_settle` Maturity Behavior:** Explicitly documented that maturity gate also applies to partial settlements

#### Document 2: `/workspaces/KARIS-KY/docs/escrow-fund-parameters.md`

**Updated State (New):**
Added warning box at the top of the document linking to comprehensive maturity documentation:

```markdown
⚠ Critical: Maturity Checks Use Ledger Timestamp

All settlement operations (`settle()` and `partial_settle()`) use the validator-observed ledger timestamp 
(`env.ledger().timestamp()`) to enforce maturity gates, NOT wall-clock time. This is essential for 
off-chain tooling and operator dashboards that monitor settlement eligibility.
```

Links readers to the detailed explanation in `escrow-lifecycle.md`.

---

### ✅ Criterion 3: `partial_settle` follows the same basis (documented)

**Status:** VERIFIED AND DOCUMENTED

#### Code-Level Verification

**File:** `/workspaces/KARIS-KY/escrow/src/settlement.rs`

The `settle_impl()` function handles **both** full and partial settlement with a single maturity check:

```rust
pub(crate) fn settle_impl(env: Env, partial_amount: Option<i128>) -> InvoiceEscrow {
    // ...
    
    let now = env.ledger().timestamp();
    // Maturity check applies to all settlements (full or partial)
    // Settlement cannot occur before the configured maturity timestamp
    if escrow.maturity > 0 {
        if now < escrow.maturity {
            fail(&env, EscrowError::MaturityNotReached);
        }
    }
    
    // Handle partial settlement (amount: Option<i128>)
    let new_settled_amount = match partial_amount {
        Some(amount) => {
            // Validate and distribute pro-rata
            current_settled_amount + amount
        }
        None => escrow.funded_amount,  // Full settlement
    };
    // ...
}
```

**Key Detail:** The maturity check **precedes** the partial/full settlement logic. 
Both paths enforce the same gate using the same timestamp source.

#### Test Verification

**Test:** `partial_settlement_respects_maturity()` (line ~2346)
- Initializes escrow with far-future maturity
- Attempts partial settlement before maturity
- **Expected:** Panic with Code 122
- **Result:** ✅ Test confirms panic occurs

**Documentation Update:**
In `/workspaces/KARIS-KY/docs/escrow-lifecycle.md`, under "Ledger-Timestamp Basis" subsection:

```markdown
#### `partial_settle` Maturity Behavior

The maturity gate also applies to partial settlements:
- `partial_settle(amount)` with `maturity > 0` enforces `env.ledger().timestamp() >= escrow.maturity`
- Once maturity is reached, **all further settlements** (full or partial) succeed without time gates
- Partial settlements do **not** reset or modify the maturity timestamp
```

---

## Implementation Details

### Source of Maturity Check

**File:** `escrow/src/settlement.rs`  
**Function:** `settle_impl(env: Env, partial_amount: Option<i128>)`  
**Lines:** ~289-313

```rust
let now = env.ledger().timestamp();  // ← Validator timestamp (UTC seconds)
if escrow.maturity > 0 {
    if now < escrow.maturity {
        // Emit diagnostic
        Self::emit_error_diagnostic(&env, diagnostic);
        // Fail with Code 122
        fail(&env, EscrowError::MaturityNotReached);
    }
}
```

### Error Code

| Code | Constant | Message |
|------|----------|---------|
| 122 | `EscrowError::MaturityNotReached` | "Escrow has not reached maturity yet" |
| (Diagnostic context) | (n/a) | "Maturity timestamp: {maturity} (in ~{seconds_remaining} seconds)" |

### Boundary Semantics

**Comparison:** `ledger_timestamp < maturity`

| Scenario | ledger_timestamp | maturity | Comparison | Result |
|----------|-----------------|----------|-----------|--------|
| Before maturity | 19_999 | 20_000 | 19_999 < 20_000 ✓ | Fail (Code 122) ✗ |
| **At maturity** | **20_000** | **20_000** | **20_000 < 20_000 ✗** | **Succeed ✓** |
| After maturity | 20_001 | 20_000 | 20_001 < 20_000 ✗ | Succeed ✓ |

**Key:** Maturity is **inclusive** — at exact maturity timestamp, settlement succeeds.

---

## Test Suite Summary

### Comprehensive Boundary Tests

| Test Name | Location | Assertion | Status |
|-----------|----------|-----------|--------|
| `settle_one_second_before_maturity_traps_and_preserves_state()` | Line ~724 | Panics at maturity - 1 | ✅ |
| `settle_at_maturity_succeeds()` | Line ~780 | Succeeds at maturity | ✅ |
| `settle_one_second_after_maturity_succeeds()` | Line ~921 | Succeeds after maturity | ✅ |
| `settle_with_maturity_zero_succeeds_immediately()` | Line ~664 | Succeeds with maturity == 0 | ✅ |
| `partial_settlement_respects_maturity()` | Line ~2346 | Panics before maturity (partial) | ✅ |

### Additional Coverage

- `settle_requires_sme_auth()` — Auth gating
- `settle_on_open_escrow_panics()` — Status guards
- `settle_on_withdrawn_escrow_panics()` — Terminal state guards
- `settle_blocked_by_legal_hold()` — Legal hold blocking
- `test_claim_gating_exact_timestamp()` — Claim lock boundary (separate from maturity)
- `test_claim_gating_with_multiple_investors()` — Multi-investor maturity behavior

---

## Operator Guidance

### For On-Chain Monitoring

1. **Settlement eligibility check:** Compare `escrow.maturity` to `env.ledger().timestamp()`
2. **Never use wall-clock time:** The validator's ledger timestamp is authoritative
3. **Boundary:** Settlement succeeds when `ledger_timestamp >= maturity`
4. **Account for clock skew:** Small clock differences between systems; use Stellar ledger as source of truth

### For Off-Chain Indexers

1. **Fetch maturity timestamp:** Read `escrow.maturity` from contract state
2. **Track ledger progression:** Monitor `ledger.timestamp()` as new blocks arrive
3. **Mark settlement ready:** When `ledger.timestamp() >= maturity`, escrow is settlement-eligible
4. **Emit alerts:** Notify operators when maturity is reached

### For Dashboards

1. **Display maturity:** Show configured maturity timestamp and current ledger timestamp
2. **Countdown:** Calculate `maturity - ledger.timestamp()` to show time remaining
3. **Boundary tolerance:** Use exact boundary (`>=`), not approximate ranges
4. **Consistency:** Always pull ledger timestamp from the escrow contract, not client clocks

---

## Documentation Changes

### File 1: `docs/escrow-lifecycle.md`

**Section:** "Maturity gate" (line 290)

**Changes:**
- Existing statement about `env.ledger().timestamp()` preserved
- Added new subsection: "Ledger-Timestamp Basis (Critical for Operators)"
  - Explains validator-observed timestamp semantics
  - Provides code snippet from `settle_impl`
  - Documents boundary behavior with test references
  - Lists 4 key implications for operators
  - Explicitly documents `partial_settle` maturity behavior

### File 2: `docs/escrow-fund-parameters.md`

**Section:** Document header (line 1)

**Changes:**
- Added warning box highlighting maturity ledger-timestamp basis
- Linked to detailed documentation in `escrow-lifecycle.md`
- Positioned prominently for SDK integrators and operators

---

## Verification Checklist

- [x] Confirmed `settle_impl()` uses `env.ledger().timestamp()` (not wall-clock)
- [x] Verified error code 122 is emitted when `now < maturity`
- [x] Confirmed maturity boundary is inclusive (`>=` semantics)
- [x] Verified tests cover: before boundary, at boundary, after boundary
- [x] Confirmed `partial_settle()` uses same maturity check
- [x] Added test for `partial_settlement_respects_maturity()`
- [x] Updated `docs/escrow-lifecycle.md` with comprehensive maturity documentation
- [x] Updated `docs/escrow-fund-parameters.md` with maturity warning
- [x] Documented ledger-timestamp basis for operators
- [x] Documented all boundary behavior and implications
- [x] Linked documentation for discoverability

---

## Related Issues and Tasks

- **Task:** Confirm settle enforces MaturityNotReached = 122 against ledger timestamp
- **Task:** Add boundary tests (already exist in codebase)
- **Task:** Document ledger-timestamp basis in docs (completed)
- **Acceptance:** All three criteria met and verified

---

## Conclusion

**Status: COMPLETE ✅**

The escrow smart contract enforces maturity exclusively against `env.ledger().timestamp()` 
(validator-observed Stellar ledger timestamp), ensuring deterministic settlement behavior 
across all nodes. The boundary is inclusive: at or after the maturity timestamp, settlement 
succeeds. This behavior is now explicitly documented for operators and off-chain tooling, 
removing ambiguity when comparing on-chain maturity to external clocks.

Both `settle()` and `partial_settle()` enforce the same maturity gate using the same 
timestamp source, confirmed by comprehensive boundary tests and now fully documented 
in operator-facing guides.
