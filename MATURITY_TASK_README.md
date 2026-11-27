# Maturity Ledger Timestamp — Implementation Overview

**Task:** Confirm and document that escrow maturity is enforced against `env.ledger().timestamp()` exclusively, with comprehensive boundary tests and operator guidance.

---

## Quick Summary

| Aspect | Status | Reference |
|--------|--------|-----------|
| **Code uses `env.ledger().timestamp()`** | ✅ Verified | `escrow/src/settlement.rs:289` |
| **Error code 122 on `maturity - 1`** | ✅ Verified | Test: `settle_one_second_before_maturity_traps_and_preserves_state()` |
| **Success at exact maturity** | ✅ Verified | Test: `settle_at_maturity_succeeds()` |
| **Success after maturity** | ✅ Verified | Test: `settle_one_second_after_maturity_succeeds()` |
| **Partial settlement respects maturity** | ✅ Verified | Test: `partial_settlement_respects_maturity()` |
| **Documentation explains ledger-timestamp basis** | ✅ Complete | `docs/escrow-lifecycle.md` + `docs/escrow-fund-parameters.md` |

---

## Key Files

### Implementation
- **`escrow/src/settlement.rs`** — Contains `settle_impl()` with maturity check at line ~289
  - Uses: `let now = env.ledger().timestamp();`
  - Gate: `if now < escrow.maturity { fail(..., Code 122) }`

### Tests
- **`escrow/src/tests/settlement.rs`** — Contains four boundary tests:
  1. `settle_one_second_before_maturity_traps_and_preserves_state()` (line ~724)
  2. `settle_at_maturity_succeeds()` (line ~780)
  3. `settle_one_second_after_maturity_succeeds()` (line ~921)
  4. `partial_settlement_respects_maturity()` (line ~2346)

### Documentation
- **`docs/escrow-lifecycle.md`** — Enhanced "Maturity gate" section with:
  - New subsection: "Ledger-Timestamp Basis (Critical for Operators)"
  - Code snippet from `settle_impl()`
  - Boundary behavior examples
  - Test references
  - 4 key implications for operators
  - `partial_settle` maturity behavior

- **`docs/escrow-fund-parameters.md`** — New critical warning box:
  - Highlights ledger-timestamp basis
  - Links to detailed docs
  - Positioned for SDK integrators and operators

### Verification Reports
- **`MATURITY_LEDGER_TIMESTAMP_VERIFICATION.md`** — Comprehensive verification document
  - All acceptance criteria verified
  - Test evidence with line references
  - Boundary semantics table
  - Operator guidance
  - Change summary

- **`MATURITY_TASK_COMPLETION_SUMMARY.md`** — Task completion report
  - Deliverables overview
  - Detailed verification of each criterion
  - Files modified summary
  - Impact analysis

---

## Boundary Behavior (Critical)

**Maturity check:** `ledger_timestamp < maturity`

| Scenario | Comparison | Settlement | Error |
|----------|-----------|-----------|-------|
| `ledger_timestamp = maturity - 1` | True | ✗ Fails | Code 122 |
| **`ledger_timestamp = maturity`** | **False** | **✓ Succeeds** | None |
| `ledger_timestamp = maturity + 1` | False | ✓ Succeeds | None |

**Key Detail:** Maturity boundary is **inclusive** — at or after the configured maturity timestamp, 
settlement is permitted. This is verified by the three boundary tests.

---

## Operator Guidance

### Settlement Eligibility Check

To determine if an escrow is ready for settlement:

```
Settlement ready when: ledger_timestamp >= maturity
```

**Where to get `ledger_timestamp`:**
- From the escrow contract: `env.ledger().timestamp()`
- From Stellar network state (not system clock)
- **Never** compare to local/UTC wall-clock time

### For Dashboards & Monitoring

1. Query escrow's maturity timestamp
2. Query Stellar ledger's current timestamp
3. Compare: `ledger_timestamp >= maturity`?
4. If yes: "Settlement ready" ✓
5. If no: Show countdown to maturity

### For Off-Chain Indexers

1. Subscribe to Stellar ledger updates
2. Track the ledger timestamp progression
3. When `ledger_timestamp >= maturity`, emit settlement-ready event
4. Use Stellar's ledger timestamp as source of truth (not external clocks)

### Common Pitfalls to Avoid

❌ **Don't:** Compare on-chain maturity to system clock  
✅ **Do:** Compare to Stellar ledger timestamp

❌ **Don't:** Assume clock drift between systems is irrelevant  
✅ **Do:** Always use the escrow contract's `env.ledger().timestamp()` as authoritative

❌ **Don't:** Trust external time sources for settlement eligibility  
✅ **Do:** Rely solely on the validator-observed ledger timestamp

---

## Code Examples

### In Rust (Contract)

```rust
// This is what the contract does (from settlement.rs):
let now = env.ledger().timestamp();  // ← Validator-observed timestamp
if escrow.maturity > 0 {
    if now < escrow.maturity {
        fail(&env, EscrowError::MaturityNotReached);  // Code 122
    }
}
// If we get here, settlement can proceed
```

### Test Example

```rust
// Test: Settle one second before maturity (should fail)
env.ledger().with_mut(|l| l.timestamp = maturity - 1);
let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    client.settle();
}));
assert!(result.is_err(), "Must panic before maturity");

// Test: Settle at maturity (should succeed)
env.ledger().with_mut(|l| l.timestamp = maturity);
let settled = client.settle();
assert_eq!(settled.status, 2);  // Status = settled
```

---

## Partial Settlement Behavior

**Key Point:** `partial_settle()` enforces the **same maturity gate** as full `settle()`.

**Timeline:**
- Escrow initialized with `maturity = T`
- Multiple investors fund the escrow
- At `ledger_timestamp < T`: Both `settle()` and `partial_settle()` are **blocked** with Code 122
- At `ledger_timestamp >= T`: Both `settle()` and `partial_settle()` succeed
- Once maturity is reached, all settlements (full or partial) proceed without time gates

**This is documented in:** `docs/escrow-lifecycle.md` under "`partial_settle` Maturity Behavior"

---

## Error Code Reference

| Code | Constant | Meaning | Recovery |
|------|----------|---------|----------|
| 122 | `MaturityNotReached` | Escrow not yet mature (settlement blocked) | Wait until maturity timestamp or check ledger timestamp |

**Diagnostic context included:** Shows the target maturity timestamp and seconds remaining.

---

## Related Documentation

For comprehensive information:

1. **Operator-focused:** [`docs/escrow-lifecycle.md` — Maturity Gate section](docs/escrow-lifecycle.md#maturity-gate)
   - Boundary behavior with test references
   - Implications for operators
   - Off-chain tooling guidance

2. **SDK integrator-focused:** [`docs/escrow-fund-parameters.md` — Maturity warning](docs/escrow-fund-parameters.md)
   - Critical notice about ledger timestamp
   - Links to detailed documentation

3. **Technical verification:** [`MATURITY_LEDGER_TIMESTAMP_VERIFICATION.md`](MATURITY_LEDGER_TIMESTAMP_VERIFICATION.md)
   - Complete test evidence
   - Code references
   - Boundary behavior tables

4. **Task completion:** [`MATURITY_TASK_COMPLETION_SUMMARY.md`](MATURITY_TASK_COMPLETION_SUMMARY.md)
   - Acceptance criteria verification
   - Files modified
   - Impact analysis

---

## Testing Maturity Locally

To run the boundary tests locally:

```bash
cd escrow

# Run maturity-specific tests
cargo test settle_one_second_before_maturity --lib tests::settlement::
cargo test settle_at_maturity_succeeds --lib tests::settlement::
cargo test settle_one_second_after_maturity --lib tests::settlement::
cargo test partial_settlement_respects_maturity --lib tests::settlement::

# Or run all settlement tests
cargo test --lib tests::settlement::
```

---

## Summary of Changes

### Code (No Changes)
✅ Implementation already correct — uses `env.ledger().timestamp()`

### Tests (Already Present)
✅ All four boundary tests exist and verify acceptance criteria

### Documentation (Enhanced)
✅ `docs/escrow-lifecycle.md` — Added "Ledger-Timestamp Basis" subsection  
✅ `docs/escrow-fund-parameters.md` — Added critical warning box  
✅ Created verification and completion reports

### Outcome
✅ **Ambiguity removed** — Operators now have crystal-clear guidance  
✅ **Boundary behavior documented** — With test references  
✅ **Off-chain integration guidance provided** — For dashboards and indexers

---

## Questions & Answers

**Q: Does maturity use wall-clock time or ledger timestamp?**  
A: **Ledger timestamp exclusively.** The validator-observed `env.ledger().timestamp()` is the single source of truth.

**Q: What happens exactly at the maturity timestamp?**  
A: Settlement **succeeds**. The boundary is inclusive: at or after maturity, settlement is permitted.

**Q: Does partial_settle have a different maturity gate?**  
A: **No.** Both `settle()` and `partial_settle()` enforce the same maturity check using the same timestamp source.

**Q: If my system clock differs from the ledger, whose timestamp do I use?**  
A: **Always use the Stellar ledger timestamp.** The contract's `env.ledger().timestamp()` is authoritative.

**Q: Can an admin override maturity enforcement?**  
A: **No.** Maturity is immutable after initialization. Extensions require explicit timelock and guardian confirmation.

**Q: How do I know my escrow is ready for settlement?**  
A: Compare: `ledger_timestamp >= escrow.maturity`. If true, settlement is eligible.

---

## Glossary

- **`env.ledger().timestamp()`** — Validator-observed Stellar ledger timestamp (UTC seconds)
- **Maturity** — Configured timestamp before which settlement is blocked
- **Maturity gate** — The check: `if now < maturity { fail }`
- **Boundary** — The exact maturity timestamp (inclusive for settlement)
- **Error Code 122** — `MaturityNotReached` — settlement attempted before maturity
- **Partial settlement** — Settling a portion of principal; still subject to maturity gate

---

## Conclusion

The escrow contract **enforces maturity exclusively against the validator-observed ledger timestamp** 
(`env.ledger().timestamp()`), ensuring deterministic settlement behavior across all network nodes. 
This behavior is now comprehensively documented with boundary-confirming tests and operator guidance, 
removing all ambiguity about wall-clock vs. ledger timestamp for off-chain integration.

**All acceptance criteria met. Task complete.** ✅
