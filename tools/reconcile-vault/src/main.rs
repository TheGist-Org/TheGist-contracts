mod aggregate;
mod events;
mod rpc;

use std::env;

use events::VaultEvent;

/// Reconciliation tool for GistVault — reference implementation.
///
/// Cross-checks GistTipped/TipsClaimed events against on-chain contract state.
/// This is a verification harness for TheGist-API to build on, NOT a production indexer.
///
/// Usage:
///   reconcile-vault <RPC_URL> <CONTRACT_ID> <START_LEDGER> <END_LEDGER>
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 5 {
        eprintln!(
            "Usage: {} <RPC_URL> <CONTRACT_ID> <START_LEDGER> <END_LEDGER>",
            args[0]
        );
        std::process::exit(1);
    }

    let rpc_url = &args[1];
    let contract_id = &args[2];
    let start_ledger: u32 = args[3].parse().expect("START_LEDGER must be a u32");
    let end_ledger: u32 = args[4].parse().expect("END_LEDGER must be a u32");

    if start_ledger > end_ledger {
        eprintln!("Error: START_LEDGER must be <= END_LEDGER");
        std::process::exit(1);
    }

    println!("=== GistVault Reconciliation Tool (reference) ===");
    println!();
    println!("  RPC:          {}", rpc_url);
    println!("  Contract:     {}", contract_id);
    println!("  Ledger range: {} - {}", start_ledger, end_ledger);
    println!();

    let client = rpc::RpcClient::new(rpc_url);

    println!("Fetching GistTipped events...");
    let all_events = client
        .fetch_events(contract_id, "tipped", start_ledger, end_ledger)
        .await?;
    let tipped_events: Vec<_> = all_events.into_iter().collect();
    println!("  Found {} GistTipped events", tipped_events.len());

    println!("Fetching TipsClaimed events...");
    let all_claimed = client
        .fetch_events(contract_id, "claimed", start_ledger, end_ledger)
        .await?;
    let claimed_events: Vec<_> = all_claimed.into_iter().collect();
    println!("  Found {} TipsClaimed events", claimed_events.len());
    println!();

    let tip_totals = aggregate::aggregate_tips(&tipped_events);
    let claim_totals = aggregate::aggregate_claims(&claimed_events);
    let pending_balances = aggregate::compute_pending_balances(&tipped_events, &claimed_events);

    println!("--- Derived State from Events ---");
    println!();
    println!("  Per-gist gross tip totals (GistTotalTips):");
    if tip_totals.is_empty() {
        println!("    (none)");
    } else {
        for (gist_id, total) in &tip_totals {
            println!("    gist {}: {} stroops", gist_id, total);
        }
    }
    println!();

    println!("  Per-author pending balances (PendingBalance):");
    if pending_balances.is_empty() {
        println!("    (none)");
    } else {
        for (author, balance) in &pending_balances {
            println!("    {}: {} stroops", author, balance);
        }
    }
    println!();

    println!("  Per-author total claims:");
    if claim_totals.is_empty() {
        println!("    (none)");
    } else {
        for (author, total) in &claim_totals {
            println!("    {}: {} stroops", author, total);
        }
    }
    println!();

    println!("--- Reconciliation ---");
    println!();
    let mut drift_detected = false;

    for (i, event) in tipped_events.iter().enumerate() {
        if let VaultEvent::Tipped(t) = event {
            if t.amount != t.fee + t.net {
                println!(
                    "  DRIFT: event #{} (txn {}): amount={} != fee({}) + net({})",
                    i + 1,
                    t.txn_hash,
                    t.amount,
                    t.fee,
                    t.net
                );
                drift_detected = true;
            }
        }
    }

    for (author, balance) in &pending_balances {
        let claimed = claim_totals.get(author).copied().unwrap_or(0);
        let tipped_net: i128 = tipped_events
            .iter()
            .filter_map(|e| match e {
                VaultEvent::Tipped(t) if &t.recipient == author => Some(t.net),
                _ => None,
            })
            .sum();
        if tipped_net - claimed != *balance {
            println!(
                "  DRIFT: author {}: sum(net)={} - claimed={} != pending={}",
                author, tipped_net, claimed, balance
            );
            drift_detected = true;
        }
    }

    if !drift_detected {
        println!("  All event-derived invariants hold. No drift detected.");
    }

    println!();
    println!("--- Next Steps ---");
    println!();
    println!("  To verify against live on-chain state, use the stellar CLI:");
    println!();
    println!("  For each gist in the table above:");
    println!("    stellar contract invoke \\");
    println!("      --id {} \\", contract_id);
    println!("      --source admin \\");
    println!("      -- \\");
    println!("      get_total_tips_for_gist --gist_id <GIST_ID>");
    println!();
    println!("  For each author:");
    println!("    stellar contract invoke \\");
    println!("      --id {} \\", contract_id);
    println!("      --source <AUTHOR_ACCOUNT> \\");
    println!("      -- \\");
    println!("      get_pending_balance --recipient <AUTHOR_ADDRESS>");
    println!();
    println!("  Compare the CLI output against the derived state above.");
    println!("  Any mismatch indicates contract drift or an indexer bug.");

    if drift_detected {
        std::process::exit(1);
    }

    Ok(())
}
