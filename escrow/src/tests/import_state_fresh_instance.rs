//! Import-safety tests: `import_state` must reject a non-fresh instance (issue #892).
//!
//! Per the README and `docs/escrow-state-export-import.md`, `import_state` is
//! designed exclusively for use on a **fresh, uninitialized** contract instance.
//! Calling it on an already-initialized contract would silently clobber live
//! investor state, so the entrypoint must reject any call where
//! `DataKey::Escrow` already exists in instance storage.
//!
//! # What this module covers
//!
//! 1. **Rejection test** — initialise an instance, then attempt `import_state`:
//!    must fail with `EscrowAlreadyInitialized` (error code 3) rather than
//!    overwriting the live escrow.
//! 2. **Happy-path test** — import onto a fresh (never-initialised) instance:
//!    must succeed and `get_escrow` must reflect the snapshot exactly.
//!
//! These two tests are the acceptance criteria for issue #892 and complement
//! the more comprehensive round-trip tests in `state_export_import.rs`.

use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Env, String as SorobanString,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Deploy and fully initialise a source escrow, return its client together
/// with the addresses used so callers can build a valid `EscrowStateExport`.
fn init_source(
    env: &Env,
) -> (
    super::LiquifactEscrowClient<'_>,
    Address, // admin
    Address, // sme
    Address, // token
    Address, // treasury
) {
    env.mock_all_auths();

    let mut li = env.ledger().get();
    li.timestamp = 1_000;
    li.sequence_number = 50;
    env.ledger().set(li);

    let escrow_id = super::deploy_id(env);
    let client = super::LiquifactEscrowClient::new(env, &escrow_id);

    let admin = Address::generate(env);
    let sme = Address::generate(env);
    let (token, treasury) = super::free_addresses(env);

    client.init(
        &admin,
        &SorobanString::from_str(env, "INV_IMPORT_SAFETY"),
        &sme,
        &50_000i128,
        &500i64, // 5% yield
        &0u64,   // no maturity lock
        &token,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    (client, admin, sme, token, treasury)
}

// ---------------------------------------------------------------------------
// Test 1: import onto an already-initialized instance must be rejected
// ---------------------------------------------------------------------------

/// Importing a snapshot onto an **already-initialized** escrow instance must
/// fail with `EscrowAlreadyInitialized` (error code 3).
///
/// # Rationale
///
/// `import_state` is specified as an emergency-use entrypoint for disaster
/// recovery onto a *fresh* instance.  Allowing an import onto a live escrow
/// would silently overwrite funded balances, investor contributions, and
/// settlement state — an irreversible data loss that cannot be detected or
/// reversed on-chain.  The guard must fire before any storage is mutated.
///
/// # Acceptance criterion (issue #892)
///
/// > Import onto an initialized instance fails with EscrowAlreadyInitialized = 3.
#[test]
fn import_state_onto_initialized_instance_fails_with_already_initialized() {
    let env = Env::default();
    // Initialise the source escrow and obtain a valid export.
    let (src, _admin, _sme, _token, _treasury) = init_source(&env);
    let export = src.export_state();

    // `src` is already initialized — importing onto it must be rejected.
    let result = src.try_import_state(&export);

    // The contract must return EscrowAlreadyInitialized = 3, not succeed or
    // panic with an untyped trap.
    super::assert_contract_error(result, super::EscrowError::EscrowAlreadyInitialized);
}

/// Importing onto an already-funded (status 1) instance must also be rejected
/// with the same `EscrowAlreadyInitialized` guard — the guard fires on the
/// presence of `DataKey::Escrow`, not on status.
#[test]
fn import_state_onto_funded_instance_is_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let token = super::install_stellar_asset_token(&env);

    // Deploy, init, and fund the source escrow.
    let src_id = super::deploy_id(&env);
    let src = super::LiquifactEscrowClient::new(&env, &src_id);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let treasury = Address::generate(&env);
    let target = 20_000i128;

    src.init(
        &admin,
        &SorobanString::from_str(&env, "INV_FUNDED_IMPORT"),
        &sme,
        &target,
        &300i64,
        &0u64,
        &token.id,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let investor = Address::generate(&env);
    token.stellar.mint(&investor, &target);
    token.token.approve(&investor, &src_id, &target, &999_999);
    src.fund(&investor, &target);

    // Export from the now-funded source (status == 1).
    let export = src.export_state();
    assert_eq!(export.escrow.status, 1);

    // Attempting to import onto the already-initialized src must still be
    // rejected with EscrowAlreadyInitialized = 3.
    let result = src.try_import_state(&export);
    super::assert_contract_error(result, super::EscrowError::EscrowAlreadyInitialized);
}

/// Importing onto an already-settled (status 2) instance must also be rejected.
/// A settled escrow is fully initialized; its `DataKey::Escrow` exists and the
/// guard must fire regardless of status.
#[test]
fn import_state_onto_settled_instance_is_rejected() {
    let env = Env::default();
    let (src, _admin, _sme, _token, _treasury) = init_source(&env);

    // Bring the escrow to status 2 (settled).
    let investor = Address::generate(&env);
    src.fund(&investor, &50_000i128);
    src.settle();
    assert_eq!(src.get_escrow().status, 2);

    let export = src.export_state();

    // Must reject — settled instance is initialized.
    let result = src.try_import_state(&export);
    super::assert_contract_error(result, super::EscrowError::EscrowAlreadyInitialized);
}

// ---------------------------------------------------------------------------
// Test 2: import onto a fresh instance succeeds and restores state
// ---------------------------------------------------------------------------

/// Importing onto a **fresh, never-initialized** instance must succeed and
/// `get_escrow` must return exactly the snapshot that was exported.
///
/// # Acceptance criterion (issue #892)
///
/// > Import onto a fresh instance restores state correctly.
#[test]
fn import_state_onto_fresh_instance_succeeds_and_get_escrow_reflects_snapshot() {
    let env = Env::default();
    let (src, _admin, _sme, token, treasury) = init_source(&env);

    let export = src.export_state();
    let original = src.get_escrow();

    // Deploy a brand-new contract instance and import the snapshot.
    let fresh_id = super::deploy_id(&env);
    let fresh = super::LiquifactEscrowClient::new(&env, &fresh_id);

    // Import must succeed without error.
    fresh.import_state(&export);

    // `get_escrow` on the fresh instance must reflect the exported snapshot.
    let restored = fresh.get_escrow();
    assert_eq!(
        restored.yield_bps, original.yield_bps,
        "yield_bps must be preserved after import"
    );
    assert_eq!(
        restored.status, original.status,
        "status must be preserved after import"
    );
    assert_eq!(
        restored.funded_amount, original.funded_amount,
        "funded_amount must be preserved after import"
    );
    assert_eq!(
        restored.funding_target, original.funding_target,
        "funding_target must be preserved after import"
    );
    assert_eq!(
        restored.maturity, original.maturity,
        "maturity must be preserved after import"
    );

    // Supporting read entrypoints must also reflect the imported state.
    assert_eq!(
        fresh.get_funding_token(),
        token,
        "funding token must match the exported snapshot"
    );
    assert_eq!(
        fresh.get_treasury(),
        treasury,
        "treasury must match the exported snapshot"
    );
    assert_eq!(
        fresh.get_version(),
        export.schema_version,
        "schema version must be preserved after import"
    );
    assert_eq!(
        fresh.get_unique_funder_count(),
        0,
        "unique funder count must be preserved (0 for a never-funded escrow)"
    );
    assert!(
        !fresh.get_legal_hold(),
        "legal hold must be preserved (false for this snapshot)"
    );
}

/// Importing an open-status (status 0) snapshot onto a fresh instance
/// leaves the escrow in the open state — not in funded or settled state.
#[test]
fn import_state_restores_open_status_correctly() {
    let env = Env::default();
    let (src, _admin, _sme, _token, _treasury) = init_source(&env);

    let export = src.export_state();
    assert_eq!(export.escrow.status, 0, "source must be in open state");

    let fresh_id = super::deploy_id(&env);
    let fresh = super::LiquifactEscrowClient::new(&env, &fresh_id);
    fresh.import_state(&export);

    assert_eq!(
        fresh.get_escrow().status,
        0,
        "imported escrow must have status 0 (open)"
    );
}

/// Importing a snapshot that includes a legal hold must restore the hold flag
/// so the target escrow is immediately protected.
#[test]
fn import_state_restores_legal_hold_flag() {
    let env = Env::default();
    let (src, _admin, _sme, _token, _treasury) = init_source(&env);

    // Activate a legal hold on the source before exporting.
    src.set_legal_hold(&true);
    assert!(src.get_legal_hold(), "pre-condition: legal hold must be active");

    let export = src.export_state();
    assert!(export.legal_hold, "export must carry the legal hold flag");

    let fresh_id = super::deploy_id(&env);
    let fresh = super::LiquifactEscrowClient::new(&env, &fresh_id);
    fresh.import_state(&export);

    assert!(
        fresh.get_legal_hold(),
        "imported instance must have the legal hold flag set"
    );
}

/// Importing a snapshot that includes a funded-close snapshot must restore it
/// so that claim and payout math work correctly on the target.
#[test]
fn import_state_restores_funding_close_snapshot() {
    let env = Env::default();
    env.mock_all_auths();

    let token = super::install_stellar_asset_token(&env);
    let src_id = super::deploy_id(&env);
    let src = super::LiquifactEscrowClient::new(&env, &src_id);
    let admin = Address::generate(&env);
    let sme = Address::generate(&env);
    let treasury = Address::generate(&env);
    let target = 25_000i128;

    src.init(
        &admin,
        &SorobanString::from_str(&env, "INV_SNAP_IMPORT"),
        &sme,
        &target,
        &400i64,
        &0u64,
        &token.id,
        &None,
        &treasury,
        &None,
        &None,
        &None,
        &None,
        &None,
        &None,
    );

    let investor = Address::generate(&env);
    token.stellar.mint(&investor, &target);
    token.token.approve(&investor, &src_id, &target, &999_999);
    src.fund(&investor, &target);

    let export = src.export_state();
    let original_snap = src
        .get_funding_close_snapshot()
        .expect("funding-close snapshot must exist after fund");

    let fresh_id = super::deploy_id(&env);
    let fresh = super::LiquifactEscrowClient::new(&env, &fresh_id);
    fresh.import_state(&export);

    let restored_snap = fresh
        .get_funding_close_snapshot()
        .expect("funding-close snapshot must be restored after import");

    assert_eq!(
        restored_snap.total_principal, original_snap.total_principal,
        "total_principal must be preserved in the funding-close snapshot"
    );
    assert_eq!(
        restored_snap.funding_target, original_snap.funding_target,
        "funding_target must be preserved in the funding-close snapshot"
    );
}

/// A subsequent `import_state` onto an instance that was previously imported
/// onto must also be rejected — the guard fires as soon as `DataKey::Escrow`
/// exists, regardless of how it got there.
#[test]
fn second_import_onto_already_imported_instance_is_rejected() {
    let env = Env::default();
    let (src, _admin, _sme, _token, _treasury) = init_source(&env);
    let export = src.export_state();

    // First import — succeeds.
    let fresh_id = super::deploy_id(&env);
    let fresh = super::LiquifactEscrowClient::new(&env, &fresh_id);
    fresh.import_state(&export);
    assert_eq!(fresh.get_escrow().status, 0);

    // Second import onto the now-initialized fresh instance must be rejected.
    let result = fresh.try_import_state(&export);
    super::assert_contract_error(result, super::EscrowError::EscrowAlreadyInitialized);
}
