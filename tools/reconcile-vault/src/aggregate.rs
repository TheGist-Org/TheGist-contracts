use std::collections::HashMap;

use crate::events::VaultEvent;

/// Compute per-gist gross tip totals from GistTipped events.
/// Returns: gist_id → total amount (gross).
pub fn aggregate_tips(events: &[VaultEvent]) -> HashMap<u64, i128> {
    let mut totals = HashMap::new();
    for event in events {
        if let VaultEvent::Tipped(t) = event {
            *totals.entry(t.gist_id).or_insert(0) += t.amount;
        }
    }
    totals
}

/// Compute per-author total claims from TipsClaimed events.
/// Returns: recipient → total claimed amount.
pub fn aggregate_claims(events: &[VaultEvent]) -> HashMap<String, i128> {
    let mut totals = HashMap::new();
    for event in events {
        if let VaultEvent::Claimed(c) = event {
            *totals.entry(c.recipient.clone()).or_insert(0) += c.amount;
        }
    }
    totals
}

/// Compute per-author pending balances from events.
/// pending = sum(net from GistTipped) - sum(amount from TipsClaimed)
/// Returns: author → pending balance.
pub fn compute_pending_balances(
    tipped: &[VaultEvent],
    claimed: &[VaultEvent],
) -> HashMap<String, i128> {
    let mut pending: HashMap<String, i128> = HashMap::new();

    for event in tipped {
        if let VaultEvent::Tipped(t) = event {
            *pending.entry(t.recipient.clone()).or_insert(0) += t.net;
        }
    }

    for event in claimed {
        if let VaultEvent::Claimed(c) = event {
            *pending.entry(c.recipient.clone()).or_insert(0) -= c.amount;
        }
    }

    pending
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{ClaimedEvent, TippedEvent};

    fn tipped(gist_id: u64, recipient: &str, amount: i128, fee: i128, net: i128) -> VaultEvent {
        VaultEvent::Tipped(TippedEvent {
            txn_hash: "tx1".to_string(),
            ledger: 100,
            gist_id,
            recipient: recipient.to_string(),
            amount,
            fee,
            net,
        })
    }

    fn claimed(recipient: &str, amount: i128) -> VaultEvent {
        VaultEvent::Claimed(ClaimedEvent {
            txn_hash: "tx2".to_string(),
            ledger: 200,
            recipient: recipient.to_string(),
            amount,
        })
    }

    #[test]
    fn test_aggregate_tips() {
        let events = vec![
            tipped(1, "A", 1_000_000, 0, 1_000_000),
            tipped(1, "B", 2_000_000, 100_000, 1_900_000),
            tipped(2, "A", 500_000, 25_000, 475_000),
        ];
        let totals = aggregate_tips(&events);
        assert_eq!(totals[&1], 3_000_000);
        assert_eq!(totals[&2], 500_000);
    }

    #[test]
    fn test_aggregate_claims() {
        let events = vec![claimed("A", 500_000), claimed("B", 1_000_000)];
        let totals = aggregate_claims(&events);
        assert_eq!(totals["A"], 500_000);
        assert_eq!(totals["B"], 1_000_000);
    }

    #[test]
    fn test_compute_pending_balances() {
        let tipped_events = vec![
            tipped(1, "A", 1_000_000, 50_000, 950_000),
            tipped(1, "A", 2_000_000, 100_000, 1_900_000),
            tipped(2, "B", 500_000, 25_000, 475_000),
        ];
        let claimed_events = vec![claimed("A", 500_000)];

        let pending = compute_pending_balances(&tipped_events, &claimed_events);
        assert_eq!(pending["A"], 2_350_000); // 950K + 1.9M - 500K
        assert_eq!(pending["B"], 475_000); // 475K - 0
    }

    #[test]
    fn test_pending_consistency() {
        // Simulate a scenario and verify that pending = net - claimed holds
        let tipped_events = vec![
            tipped(1, "alice", 3_000_000, 150_000, 2_850_000),
            tipped(2, "alice", 1_000_000, 50_000, 950_000),
        ];
        let claimed_events = vec![claimed("alice", 1_000_000)];

        let pending = compute_pending_balances(&tipped_events, &claimed_events);
        let total_net: i128 = tipped_events
            .iter()
            .filter_map(|e| match e {
                VaultEvent::Tipped(t) if t.recipient == "alice" => Some(t.net),
                _ => None,
            })
            .sum();
        let total_claimed: i128 = claimed_events
            .iter()
            .filter_map(|e| match e {
                VaultEvent::Claimed(c) if c.recipient == "alice" => Some(c.amount),
                _ => None,
            })
            .sum();

        assert_eq!(pending["alice"], total_net - total_claimed);
    }
}
