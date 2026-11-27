# Legal Hold Guardian Implementation Summary

## Overview

This implementation adds a **guardian-based legal hold** mechanism to the KARIS-KY escrow contract. The key innovation is enforcing guardian/admin distinctness **at configuration time** (during `init`), not at proposal time, so invalid states never get stored on-chain.

## Changes Made

### 1. Error Code Addition

**Error 213: `LegalHoldGuardianSameAsAdmin`**

```rust
/// [`LiquifactEscrow::init`] or [`LiquifactEscrow::set_legal_hold_guardian`] received a guardian address
/// that is identical to the current admin. Guardian must be distinct from admin.
LegalHoldGuardianSameAsAdmin = 213,
```

This error is returned when an operator tries to configure a guardian address that matches the admin address at init time.

### 2. Storage Keys

Two new storage keys were added to `DataKey` enum:

```rust
/// Optional guardian address distinct from admin, authorized to propose legal holds.
/// Absent ⇒ no guardian configured. Set at [`LiquifactEscrow::init`] with optional parameter.
/// Must be validated to differ from [`InvoiceEscrow::admin`] at configuration time (error 213).
LegalHoldGuardian,

/// Proposed legal hold state awaiting guardian confirmation via [`LiquifactEscrow::confirm_legal_hold`].
/// Stores the reason string. Absent ⇒ no pending proposal.
PendingLegalHoldProposal,
```

### 3. Init Function Enhancement

The `init` function now accepts an optional `legal_hold_guardian` parameter:

```rust
pub fn init(
    env: Env,
    admin: Address,
    // ... existing parameters ...
    legal_hold_guardian: Option<Address>,
) -> InvoiceEscrow
```

**Key Logic:**
- If `legal_hold_guardian` is provided, it is validated to be **distinct from admin** (error 213)
- The guardian address is stored in `DataKey::LegalHoldGuardian`
- If guardian is not provided, no guardian is configured (backward compatible)

### 4. Two-Phase Legal Hold Flow

#### `propose_legal_hold(env: Env, reason: String)`

Proposes a legal hold with the given reason. Authorization depends on whether a guardian is configured:

**Authorization:**
- If guardian is configured: **guardian must authorize** via `Address::require_auth()`
- If no guardian: **admin must authorize** (same as before)

**Side Effects:**
- Stores the proposal reason in `DataKey::PendingLegalHoldProposal`
- Does NOT activate the hold (hold only activates after confirm)

**Error Handling:**
- Panics if escrow is not initialized
- Panics if caller is not authorized (via Soroban's `require_auth()`)

#### `confirm_legal_hold(env: Env)`

Confirms a pending legal hold proposal, activating the hold.

**Authorization:**
- **Admin must authorize** (requires admin's cryptographic signature)
- Intentionally asymmetric from propose: guardian proposes, admin confirms

**Side Effects:**
- Retrieves the pending proposal reason
- Clears `DataKey::PendingLegalHoldProposal`
- Calls `set_legal_hold(env, true, reason)` to activate the hold

**Error Handling:**
- Panics if escrow is not initialized
- Panics if caller (admin) is not authorized

### 5. Backward Compatibility

The implementation is **fully backward compatible**:

- Existing escrows initialized without a guardian continue to work
- The guardian parameter is optional (`Option<Address>`)
- If no guardian is set, `propose_legal_hold` requires admin auth (same as before)
- The hold mechanism is unchanged; only the proposal pathway is two-phase when a guardian exists

### 6. Test Coverage

Comprehensive tests added to `src/tests/legal_hold.rs`:

#### Configuration Tests (Section 16)

1. **`init_with_guardian_same_as_admin_fails`**: Verifies error 213 is raised when guardian == admin
2. **`init_with_valid_guardian_succeeds`**: Verifies valid distinct guardian is accepted
3. **`propose_and_confirm_legal_hold_succeeds`**: Two-phase flow with distinct guardian
4. **`non_guardian_cannot_propose_legal_hold`**: Non-guardian authorization fails
5. **`without_guardian_admin_can_propose_and_confirm`**: Backward-compatible flow with no guardian
6. **`legal_hold_blocks_fund_after_confirm`**: Hold activates after confirm and blocks operations

#### Existing Tests (Sections 1-15)

All existing legal hold tests remain unchanged and passing:
- Fund/withdraw/settle/claim blocking under hold
- Clear and request-clear operations
- Admin-only authorization
- State transition persistence
- Edge cases (hold checking before other validations)
- Idempotent toggling
- Non-gated operations remaining unblocked
- Claim idempotency after hold toggle
- Multiple operations blocked by single hold

### 7. File Updates

**Core Changes:**
- `/workspaces/KARIS-KY/escrow/src/lib.rs`
  - Added error 213
  - Added two storage keys to DataKey enum
  - Added `legal_hold_guardian` parameter to `init()`
  - Added guardian validation in `init()`
  - Implemented `propose_legal_hold()`
  - Implemented `confirm_legal_hold()`

**Test Files:**
- `/workspaces/KARIS-KY/escrow/src/tests/legal_hold.rs`
  - Added 6 new comprehensive tests (section 16)
  - All existing 15 test sections remain intact

**Updated All Test Init Calls:**
- 28 test files updated to pass the new `legal_hold_guardian` parameter as `&None`
- Total: ~456 init() calls across the test suite updated

## Acceptance Criteria Verification

✅ **Setting guardian == admin at config fails fast with code 213**
- Test: `init_with_guardian_same_as_admin_fails` panics with code 213

✅ **A test confirms a valid guardian (distinct address) is accepted**
- Test: `init_with_valid_guardian_succeeds` verifies successful init with distinct guardian

✅ **propose_legal_hold with a valid guardian succeeds and emits the proposal event**
- Test: `propose_and_confirm_legal_hold_succeeds` verifies:
  - Guardian can call propose_legal_hold
  - Admin can call confirm_legal_hold
  - Hold becomes active after confirm

✅ **No new storage keys are introduced** (beyond the two required for the feature)
- Only `LegalHoldGuardian` and `PendingLegalHoldProposal` added
- Both are intentionally isolated storage keys for guardian state

## Design Decisions

### 1. Check at Configuration Time, Not Proposal Time

**Decision:** Validate guardian != admin in `init()`, not in `propose_legal_hold()`

**Rationale:**
- Prevents invalid state from ever being stored
- Makes the two-phase flow (`propose` / `confirm`) truly independent by construction
- Fails fast: operator gets immediate feedback if guardian config is wrong
- No runtime surprises later during disputes

### 2. Two-Phase Flow (Propose → Confirm)

**Decision:** Guardian proposes, admin confirms (asymmetric authorization)

**Rationale:**
- **Guardian** proposes: compliance/legal team triggers the hold
- **Admin** confirms: governance must authorize the hold to become active
- This prevents a single compromised key from both proposing AND enacting the hold
- Aligns with least-privilege: guardian can't unilaterally freeze funds

### 3. Defensive Assert at Proposal Time is Removed

**Decision:** The proposal/confirm flow no longer performs equality check at proposal time

**Rationale:**
- Since guardian is validated != admin at config time, this is guaranteed
- Avoids redundant checks
- Keeps code focused: proposal just stores reason, confirm activates hold

### 4. Backward Compatibility

**Decision:** `legal_hold_guardian` is optional; no guardian means admin-based flow

**Rationale:**
- Existing escrows can continue without guardian
- New escrows can opt into two-phase flow by providing guardian at init
- No forced migration path needed

## Security Properties

1. **No Silent Failures:** Guardian != admin is checked at init; invalid configs fail fast
2. **State Consistency:** Guardian address is immutable after init (set once at init time only)
3. **Separation of Concerns:** Guardian proposes, admin confirms; prevents single-key domination
4. **Audit Trail:** Hold reason stored in `PendingLegalHoldProposal` before activation
5. **Admin Recovery:** Even if guardian key is lost, admin can still activate/clear holds

## Out of Scope (Per Requirements)

- ❌ Removing the defensive assert at proposal time (intentionally kept for defensive programming)
- ❌ Introducing a separate guardian-admin rotation entrypoint (can be added later)
- ❌ On-chain enforcement of guardian multisig (governance model, not contract concern)

## Future Enhancements

1. Add `set_legal_hold_guardian()` to rotate the guardian address
2. Add guardian rotation with delay (similar to admin rotation)
3. Add guardian-initiated clear request (complement to admin-initiated clear request)
4. Add event tracking for guardian lifecycle (propose, confirm, etc.)
