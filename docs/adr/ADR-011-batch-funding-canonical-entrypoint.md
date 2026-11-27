# ADR-011: Canonical Batch Funding Entrypoint

**Status:** Accepted  
**Date:** 2026-02-10  
**Refs:** `escrow/src/lib.rs::LiquifactEscrow::fund_batch`, `sdk-ts/src/client.ts::EscrowClient::fundBatch`, `docs/escrow-fund-parameters.md`

---

## Context

The escrow contract supports batch funding to reduce transaction overhead during primary issuance workflows. Early iterations explored multiple naming schemes:

1. **`fund_batch`** (implemented first; stable)
2. **`batch_fund`** (considered as an alternative design)

Both naming approaches serve the same semantic function: submit multiple `(investor, amount)` tuples in a single atomic call. The existence of two parallel names or implementations risks:

- **Implementation drift:** Code paths diverge over time, leading to correctness bugs or inconsistent behavior.
- **SDK consumer confusion:** Which method should production integrations use?
- **Maintenance burden:** Two implementations double review, testing, and upgrade complexity.

## Decision

**`fund_batch` is the canonical batch funding entrypoint.**

- **Single public interface:** All batch funding requests must use `fund_batch`.
- **No duplication:** Any alternative naming variant (e.g., `batch_fund`, `batch_fund_many`, etc.) must:
  1. Never exist as a parallel implementation with separate logic.
  2. If added as a deprecated alias, must be marked with `#[deprecated]` in Rust with a message directing callers to `fund_batch`.
  3. Forward directly to `fund_batch` without reimplementing validation or state mutations.
- **SDK alignment:** The TypeScript client exposes only `fundBatch()`, which calls the canonical `fund_batch` entrypoint on the contract.

## Rationale

1. **Semantic clarity:** Batch funding is a single capability, not a choice between variants. One canonical name prevents naming ambiguity.

2. **Testing efficiency:** One implementation path means test suites are smaller and more effective. Equivalence testing (batch vs. sequential) is more tractable.

3. **Future deprecation:** If a naming variant is ever added for backward compatibility with an earlier SDK, it can be deprecated without breaking existing code—only the new variant is marked `#[deprecated]`, while `fund_batch` remains stable.

4. **Naming convention:** `fund_batch` follows the verb-noun pattern used elsewhere in the contract (`fund`, `fund_with_commitment`, `settle`, `withdraw`) and is the first-implemented name, so it has historical continuity.

## Implementation

### Rust Contract
- `pub fn fund_batch(env: Env, entries: Vec<(Address, i128)>) -> InvoiceEscrow` is the **canonical** entrypoint.
- If `batch_fund` is ever added in a future release for compatibility, it must be:
  ```rust
  #[deprecated(
      since = "<version>",
      note = "use `fund_batch` instead; this alias is deprecated for consolidation"
  )]
  pub fn batch_fund(env: Env, entries: Vec<(Address, i128)>) -> InvoiceEscrow {
      Self::fund_batch(env, entries)
  }
  ```
- Updated rustdoc for `fund_batch` explicitly labels it as canonical and discourages duplication.

### TypeScript SDK
- `async fundBatch(entries, source?)` is the only batch funding method exposed by `EscrowClient`.
- JSDoc clarifies it is the canonical interface.
- No `batchFund()` or other variants are exposed.

### Documentation
- `docs/escrow-fund-parameters.md` documents `fund_batch` as canonical and specifies the deprecation policy if a variant is ever added.
- Module-level rustdoc in `lib.rs` discourages adding alternative batch implementations.

## Acceptance Criteria

✓ Exactly one batch entrypoint (`fund_batch`) is public in the contract.  
✓ Rustdoc for `fund_batch` labels it as canonical and warns against duplication.  
✓ TypeScript client exposes only `fundBatch()` (not `batchFund`).  
✓ Documentation clearly states the deprecation policy for future variants.  
✓ `cargo doc --no-deps` produces no broken intra-doc links (when Rust toolchain is available).  
✓ All existing tests pass without modification.

## Related Decisions

- [ADR-002: Authorization Boundaries](ADR-002-auth-boundaries.md) — `fund_batch` enforces per-investor auth for each entry.
- [Escrow Lifecycle](../escrow-lifecycle.md) § Batch funding — semantics and state transitions.
- [Escrow Fund Parameters](../escrow-fund-parameters.md) — comprehensive parameter specification and deprecation policy.
