# Archive Escrow Test Organization

## Test Summary

**Location:** `escrow/src/tests/settlement.rs`  
**Total Tests:** 16 archive-related tests  
**New Tests in this PR:** 10 new tests (lines 1954–2080)  
**Existing Tests:** 5 prior tests (lines 1864–1950)  
**Regression Tests:** 1 from regressions.rs (INC-210)

---

## Test Organization by Category

### 1. Precondition Failure Tests (3 tests)

These tests verify that `archive_escrow()` correctly rejects escrows in non-terminal states.

#### `archive_escrow_rejects_open_escrow`
- **Status:** `0` (open)
- **Expected:** Panics with `EscrowError::ArchiveNotTerminal`
- **Scenario:** Escrow initialized but not funded
- **Code:** Lines 1956–1963

#### `archive_escrow_rejects_funded_escrow`
- **Status:** `1` (funded)
- **Expected:** Panics with `EscrowError::ArchiveNotTerminal`
- **Scenario:** Escrow reached funding target but not settled/withdrawn
- **Code:** Lines 1965–1976

#### `archive_escrow_requires_admin_auth`
- **Status:** `3` (withdrawn)
- **Expected:** Panics when non-admin calls archive
- **Scenario:** SME or other non-admin role attempts archive
- **Code:** Lines 1978–1992

---

### 2. Post-Archive Mutation Rejection Tests (5 tests)

These tests verify that no state-mutating entrypoint can be called on an archived (status 5) escrow.

#### `fund_on_archived_escrow_panics`
- **Operation:** `fund(investor, amount)`
- **Expected:** Panics (requires status 0)
- **Code:** Lines 1994–2004

#### `settle_on_archived_escrow_panics`
- **Operation:** `settle()`
- **Expected:** Panics (requires status 1)
- **Code:** Lines 2006–2016

#### `withdraw_on_archived_escrow_panics`
- **Operation:** `withdraw(sme)`
- **Expected:** Panics (requires status 1)
- **Code:** Lines 2018–2028

#### `claim_on_archived_escrow_panics`
- **Operation:** `claim_investor_payout(investor)`
- **Expected:** Panics or idempotent if already claimed (requires status 2)
- **Code:** Lines 2030–2040
- **Note:** This test uses `get_escrow()` to verify status is 5 after archiving

#### `rotate_beneficiary_on_archived_escrow_panics`
- **Operation:** `rotate_beneficiary(new_sme)`
- **Expected:** Panics (requires status 0 or 1)
- **Code:** Lines 2042–2052

---

### 3. Irreversibility Tests (2 tests)

These tests verify that archival is irreversible and that the status persists across multiple calls.

#### `archived_escrow_status_irreversible`
- **Precondition:** Withdraw → Archive → Archive again
- **Expected:** Status remains 5 (never reverts to 3)
- **Verification:** 
  - After first archive: status == 5
  - After second archive: status == 5 (idempotent, not reverted)
- **Code:** Lines 2054–2080

#### `get_escrow_on_archived_returns_full_state`
- **Precondition:** Archived escrow
- **Expected:** All fields returned unchanged
- **Verification:**
  - `status` == 5
  - `invoice_id` unchanged
  - `funded_amount` unchanged
  - `admin` unchanged
- **Code:** Lines 2082–2101

---

### 4. Existing Tests (from prior implementation)

#### `archive_escrow_blocked_by_unclaimed_payouts` (Line 1871)
- **Status:** `2` (settled)
- **Precondition:** One investor funded but has not claimed
- **Expected:** Panics with `ArchiveBlockedByUnclaimedPayouts`
- **Code:** Lines 1868–1879

#### `archive_escrow_succeeds_after_all_claims` (Line 1883)
- **Status:** `2` (settled)
- **Precondition:** All investors have claimed
- **Expected:** Success, status becomes 5
- **Code:** Lines 1881–1898

#### `archive_escrow_succeeds_for_withdrawn_escrow` (Line 1903)
- **Status:** `3` (withdrawn)
- **Precondition:** None (no investor liabilities)
- **Expected:** Success, status becomes 5
- **Code:** Lines 1900–1915

#### `archive_escrow_succeeds_for_cancelled_escrow` (Line 1917)
- **Status:** `4` (cancelled)
- **Precondition:** None (no investor liabilities)
- **Expected:** Success, status becomes 5
- **Code:** Lines 1914–1928

#### `archive_escrow_is_idempotent_when_already_archived` (Line 1931)
- **Status:** `5` (already archived)
- **Precondition:** Already archived (status 5)
- **Expected:** No-op, no panic, status remains 5
- **Code:** Lines 1928–1948

---

### 5. Regression Test (from regressions.rs)

#### `test_regression_210_archive_with_unclaimed_payouts`
- **Incident:** INC-210 — Escrow archived while investor payout was unclaimed
- **Precondition:** Settled escrow with unclaimed investor
- **Expected:** 
  - First archive attempt → panics (unclaimed payout)
  - After investor claims → archive succeeds
- **Code:** `escrow/src/tests/regressions.rs` (lines ~445–482)

---

## Test Execution

### Run all archive tests
```bash
cd escrow
cargo test --lib tests::settlement::archive_escrow
```

### Run specific test
```bash
cargo test --lib tests::settlement::archive_escrow_rejects_open_escrow -- --nocapture
```

### Run regression test
```bash
cargo test --lib tests::regressions::test_regression_210
```

### Run full settlement test suite
```bash
cargo test --lib tests::settlement
```

---

## Test Preconditions (Common Setup)

All tests use one of two setup patterns:

### Pattern 1: Basic setup with no funding
```rust
let (client, admin, sme) = setup(&env);
default_init(&client, &env, &admin, &sme);
```
**Result:** Escrow initialized with status 0 (open)

### Pattern 2: Funded then withdrawn
```rust
let (client, admin, sme) = setup(&env);
default_init(&client, &env, &admin, &sme);
fund_to_target(&client, &env);         // status → 1
client.withdraw(&sme);                 // status → 3
```
**Result:** Escrow withdrawn with status 3 (withdrawn)

### Pattern 3: Funded then settled
```rust
let (client, admin, sme) = setup(&env);
default_init(&client, &env, &admin, &sme);
let investor = settle_escrow(&client, &env);  // status → 2
```
**Result:** Escrow settled with status 2, investor can claim

---

## Assertion Patterns

### For precondition failures (panics)
```rust
#[test]
#[should_panic(expected = "ArchiveNotTerminal")]
fn test_name() {
    // ... setup ...
    client.archive_escrow();  // must panic with expected message
}
```

### For state verification
```rust
let escrow = client.get_escrow();
assert_eq!(escrow.status, 5u32, "status must be 5 (archived) after archive_escrow");
```

### For idempotency
```rust
client.archive_escrow();  // first call
let status1 = client.get_escrow().status;

client.archive_escrow();  // second call (idempotent)
let status2 = client.get_escrow().status;

assert_eq!(status1, status2, "status must be unchanged");
```

---

## Coverage Matrix

| Scenario | Test | Status Path | Expected Result |
|----------|------|-------------|-----------------|
| Archive open escrow | `rejects_open` | 0 | ✗ ArchiveNotTerminal |
| Archive funded escrow | `rejects_funded` | 1 | ✗ ArchiveNotTerminal |
| Archive non-admin | `requires_admin_auth` | 3 → 5 | ✗ Auth panic |
| Fund on archived | `fund_...panics` | 5 | ✗ EscrowNotOpenForFunding |
| Settle on archived | `settle_...panics` | 5 | ✗ SettlementNotFunded |
| Withdraw on archived | `withdraw_...panics` | 5 | ✗ WithdrawalNotFunded |
| Claim on archived | `claim_...panics` | 5 | ✗ InvestorClaimNotSettled |
| Rotate on archived | `rotate_...panics` | 5 | ✗ RotationNotOpen |
| Archive settled (unclaimed) | `blocked_by_unclaimed` | 2 | ✗ ArchiveBlockedByUnclaimedPayouts |
| Archive settled (all claimed) | `succeeds_after_all_claims` | 2 → 5 | ✓ Success |
| Archive withdrawn | `succeeds_for_withdrawn` | 3 → 5 | ✓ Success |
| Archive cancelled | `succeeds_for_cancelled` | 4 → 5 | ✓ Success |
| Archive archived | `is_idempotent_...archived` | 5 → 5 | ✓ No-op |
| Persist status after archive | `status_irreversible` | 3 → 5 → 5 | ✓ Status == 5 |
| Read all fields on archived | `get_escrow_..._returns` | 5 | ✓ All fields intact |

---

## Expected Failure Modes (Pre-Implementation)

Before the implementation, these tests would fail:

1. **`rejects_open_escrow`, `rejects_funded_escrow`** — Would succeed without error (no precondition check)
2. **`blocked_by_unclaimed_payouts`** — Would succeed without checking investor claims (from INC-210)
3. **`fund_on_archived`**, etc. — Would succeed on archived escrow (no status check)
4. **`status_irreversible`** — Might regress to 3 or other non-5 value

After implementation, all tests should pass.

---

## Notes for Reviewers

- Tests are ordered logically: precondition failures first, then post-archive mutations, then irreversibility
- Each test is standalone and does not depend on prior test results
- Test names follow the pattern `{target}_{condition}_{expected}` for clarity
- Both success and failure paths are covered
- Panic expectations use specific error names for type safety
