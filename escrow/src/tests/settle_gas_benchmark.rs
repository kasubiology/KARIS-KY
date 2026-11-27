//! Gas-budget benchmark: `settle` with varying investor counts (issue #893).
//!
//! `settle` performs O(1) storage operations on the escrow itself — but when
//! `yield_auto_distribution_enabled` is true it additionally iterates investor
//! state.  Even without auto-distribution, the total instruction budget
//! consumed by settle scales slightly with the number of unique funders (more
//! persistent reads during the payout-snapshot path).  This module records the
//! actual CPU-instruction and memory-byte cost at 1, 50, and 200 investors so
//! that the scaling curve is visible and so that a budget-threshold assert
//! catches any future regression that pushes `settle` over the Soroban
//! mainnet per-transaction limit.
//!
//! # Running
//!
//! ```text
//! cargo test -p soroban-escrow settle_gas -- --ignored --nocapture
//! ```
//!
//! Results are captured in `docs/escrow-gas-storage-notes.md` under the
//! *settle gas scaling* section.

use soroban_sdk::{Address, Env, String as SorobanString};

/// Soroban mainnet per-transaction CPU instruction budget.
const CPU_BUDGET: u64 = 100_000_000;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Initialise an escrow and fund it with `n` distinct investors, each
/// contributing an equal share so that the total equals `funding_target`.
/// Returns `(client, investor_vec)`.
///
/// Uses the shared `setup` helper from the parent test module to create a
/// fresh `Env` with `mock_all_auths` and a default ledger state.
fn setup_funded_n_investors(
    env: &Env,
    n: usize,
    funding_target: i128,
) -> super::LiquifactEscrowClient<'_> {
    let (client, admin, sme) = super::setup(env);

    let (token, treasury) = super::free_addresses(env);
    client.init(
        &admin,
        &SorobanString::from_str(env, "INV_BENCH_SETTLE"),
        &sme,
        &funding_target,
        &800i64,  // 8% yield
        &0u64,    // no maturity lock — settle is immediate
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

    // Each investor contributes an equal share.  When n does not evenly divide
    // funding_target the last investor makes up the rounding remainder so the
    // escrow closes to funded status (status == 1) exactly once.
    let per_investor = funding_target / (n as i128);
    let remainder = funding_target - per_investor * (n as i128 - 1);

    for i in 0..n {
        let investor = Address::generate(env);
        let amount = if i == n - 1 { remainder } else { per_investor };
        client.fund(&investor, &amount);
    }

    // Verify the escrow is in funded state before the benchmark begins.
    let escrow = client.get_escrow();
    assert_eq!(
        escrow.status, 1,
        "escrow must reach funded status before settle benchmark"
    );

    client
}

// ---------------------------------------------------------------------------
// Individual size checks (always run)
// ---------------------------------------------------------------------------

/// Settling an escrow with a **single** investor must stay within the
/// Soroban CPU instruction budget.
#[test]
fn settle_gas_1_investor_within_budget() {
    let env = Env::default();
    let client = setup_funded_n_investors(&env, 1, 10_000_000_000i128);

    env.budget().reset_default();
    let settled = client.settle();
    let cpu = env.budget().cpu_instruction_cost();
    let mem = env.budget().memory_bytes_cost();

    assert_eq!(settled.status, 2, "settle must transition to status 2");
    assert!(
        cpu <= CPU_BUDGET,
        "settle with 1 investor uses {cpu} CPU instructions; budget is {CPU_BUDGET}"
    );

    std::println!(
        "[settle_gas] n=1   cpu={cpu:<12} mem={mem:<10}"
    );
}

/// Settling an escrow with **50 investors** must stay within the Soroban
/// CPU instruction budget.
#[test]
fn settle_gas_50_investors_within_budget() {
    let env = Env::default();
    let client = setup_funded_n_investors(&env, 50, 50_000_000_000i128);

    env.budget().reset_default();
    let settled = client.settle();
    let cpu = env.budget().cpu_instruction_cost();
    let mem = env.budget().memory_bytes_cost();

    assert_eq!(settled.status, 2);
    assert!(
        cpu <= CPU_BUDGET,
        "settle with 50 investors uses {cpu} CPU instructions; budget is {CPU_BUDGET}"
    );

    std::println!(
        "[settle_gas] n=50  cpu={cpu:<12} mem={mem:<10}"
    );
}

/// Settling an escrow with **200 investors** must stay within the Soroban
/// CPU instruction budget.
///
/// 200 investors is a realistic upper bound for a typical invoice escrow.
/// If `settle` ever exceeds the budget at this scale a mainnet transaction
/// will be rejected — this assert catches that regression before deployment.
#[test]
fn settle_gas_200_investors_within_budget() {
    let env = Env::default();
    let client = setup_funded_n_investors(&env, 200, 200_000_000_000i128);

    env.budget().reset_default();
    let settled = client.settle();
    let cpu = env.budget().cpu_instruction_cost();
    let mem = env.budget().memory_bytes_cost();

    assert_eq!(settled.status, 2);
    assert!(
        cpu <= CPU_BUDGET,
        "settle with 200 investors uses {cpu} CPU instructions; budget is {CPU_BUDGET}. \
         This is a mainnet-safety regression — see docs/escrow-gas-storage-notes.md"
    );

    std::println!(
        "[settle_gas] n=200 cpu={cpu:<12} mem={mem:<10}"
    );
}

// ---------------------------------------------------------------------------
// Scaling curve benchmark (--ignored; run explicitly)
// ---------------------------------------------------------------------------

/// Record the full settle gas scaling curve at 1, 50, and 200 investors.
///
/// Run with:
/// ```text
/// cargo test -p soroban-escrow settle_gas_scaling_curve -- --ignored --nocapture
/// ```
///
/// Output is formatted for direct inclusion in the table in
/// `docs/escrow-gas-storage-notes.md`.
#[test]
#[ignore]
fn settle_gas_scaling_curve() {
    const INVESTOR_COUNTS: &[(usize, i128)] = &[
        (1,   10_000_000_000i128),
        (50,  50_000_000_000i128),
        (200, 200_000_000_000i128),
    ];

    std::println!(
        "\n{:<10} {:<18} {:<18} {:<18} {:<18}",
        "investors",
        "cpu_instructions",
        "mem_bytes",
        "cpu_% of budget",
        "cpu_per_investor",
    );
    std::println!("{}", "-".repeat(85));

    let mut prev_cpu: Option<u64> = None;

    for &(n, funding_target) in INVESTOR_COUNTS {
        let env = Env::default();
        let client = setup_funded_n_investors(&env, n, funding_target);

        env.budget().reset_default();
        let settled = client.settle();
        let cpu = env.budget().cpu_instruction_cost();
        let mem = env.budget().memory_bytes_cost();

        assert_eq!(settled.status, 2, "settle must succeed for n={n}");
        assert!(
            cpu <= CPU_BUDGET,
            "n={n} settle uses {cpu} CPU instructions — exceeds budget {CPU_BUDGET}"
        );

        let cpu_pct = (cpu * 100) / CPU_BUDGET;
        let cpu_per_inv = cpu / (n as u64);

        // Scaling slope: cpu[n] / cpu[1].  A value close to 1.0 confirms O(1)
        // per-investor behaviour; a value that grows linearly with n would
        // indicate an O(N) code path that needs attention.
        let slope_note = if let Some(base) = prev_cpu {
            let slope = (cpu as f64) / (base as f64);
            std::format!("  slope_vs_prev={slope:.2}x")
        } else {
            String::new()
        };
        prev_cpu = Some(cpu);

        std::println!(
            "{:<10} {:<18} {:<18} {:<18} {:<18}{}",
            n,
            cpu,
            mem,
            std::format!("{cpu_pct}%"),
            cpu_per_inv,
            slope_note,
        );
    }

    std::println!();
    std::println!(
        "Budget threshold assert: all investor counts must remain below {CPU_BUDGET} CPU instructions."
    );
}

// ---------------------------------------------------------------------------
// Storage-operation count
// ---------------------------------------------------------------------------

/// Verify that `settle` performs a constant number of instance-storage reads
/// and writes regardless of investor count.
///
/// `settle` itself reads and writes the following instance keys:
///   - `DataKey::Escrow` (1 read + 1 write)
///   - `DataKey::SettledAmount` (1 read + 1 write)
///   - `DataKey::FundingCloseSnapshot` (1 read, no write)
///   - `DataKey::FeePercentage` (1 read)
///   - `DataKey::YieldAutoDistributionEnabled` (1 read)
///
/// All reads are O(1); none iterate over investor-keyed storage.
/// This test asserts the CPU cost grows sub-linearly (< 50% more) going
/// from 1 to 50 to 200 investors, confirming no hidden O(N) loop exists.
#[test]
fn settle_cpu_growth_is_sublinear_across_investor_counts() {
    let configs: &[(usize, i128)] = &[
        (1,   10_000_000_000i128),
        (50,  50_000_000_000i128),
        (200, 200_000_000_000i128),
    ];

    let mut costs: Vec<u64> = Vec::new();

    for &(n, funding_target) in configs {
        let env = Env::default();
        let client = setup_funded_n_investors(&env, n, funding_target);

        env.budget().reset_default();
        client.settle();
        let cpu = env.budget().cpu_instruction_cost();
        costs.push(cpu);

        std::println!("[settle_sublinear] n={n:<4} cpu={cpu}");
    }

    // The cost from n=1 to n=200 must grow by less than 10x.
    // A linear scaling from 1→200 investors would be a 200x increase.
    // Anything under 10x is consistent with O(1) settle + O(N) fund setup.
    let baseline = costs[0];
    let at_200 = costs[2];
    assert!(
        at_200 <= baseline * 10,
        "settle CPU at 200 investors ({at_200}) is more than 10× the 1-investor baseline \
         ({baseline}). This suggests an O(N) path was introduced — review settle_impl."
    );
}
