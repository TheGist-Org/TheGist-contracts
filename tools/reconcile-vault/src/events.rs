use serde::Deserialize;

/// A decoded GistTipped or TipsClaimed event from the Soroban RPC.
#[derive(Debug, Clone)]
pub enum VaultEvent {
    Tipped(TippedEvent),
    Claimed(ClaimedEvent),
}

#[derive(Debug, Clone)]
#[allow(
    dead_code,
    reason = "fields are part of the event data model for completeness"
)]
pub struct TippedEvent {
    pub txn_hash: String,
    pub ledger: u32,
    pub gist_id: u64,
    pub recipient: String,
    pub amount: i128,
    pub fee: i128,
    pub net: i128,
}

#[derive(Debug, Clone)]
#[allow(
    dead_code,
    reason = "fields are part of the event data model for completeness"
)]
pub struct ClaimedEvent {
    pub txn_hash: String,
    pub ledger: u32,
    pub recipient: String,
    pub amount: i128,
}

/// Raw JSON data structure for GistTippedEvent.
#[derive(Deserialize)]
struct TippedData {
    gist_id: u64,
    recipient: String,
    amount: i128,
    fee: i128,
    net: i128,
}

/// Raw JSON data structure for TipsClaimedEvent.
#[derive(Deserialize)]
struct ClaimedData {
    recipient: String,
    amount: i128,
}

/// Get the second topic (event name) from the RPC event topics list.
fn second_topic(topics: &[String]) -> Option<&str> {
    topics.get(1).map(|s| s.as_str())
}

impl VaultEvent {
    /// Parse a raw Soroban RPC event into a VaultEvent.
    /// Returns Ok(None) if the event data cannot be decoded (logged but not fatal).
    pub fn from_rpc(rpc_event: &crate::rpc::RpcEvent) -> anyhow::Result<Option<Self>> {
        let event_name = match second_topic(&rpc_event.topics) {
            Some(name) => name,
            None => return Ok(None),
        };

        // The data field is base64-encoded XDR. For the reference tool, we attempt
        // to parse it as JSON first (some RPC implementations return JSON). If that
        // fails, we fall back to treating it as raw XDR (hex or base64).
        let data_str = &rpc_event.data;
        let ledger: u32 = rpc_event.ledger.parse().unwrap_or(0);

        match event_name {
            "tipped" => {
                if let Ok(data) = serde_json::from_str::<TippedData>(data_str) {
                    return Ok(Some(VaultEvent::Tipped(TippedEvent {
                        txn_hash: rpc_event.tx_hash.clone(),
                        ledger,
                        gist_id: data.gist_id,
                        recipient: data.recipient,
                        amount: data.amount,
                        fee: data.fee,
                        net: data.net,
                    })));
                }
                // XDR fallback: try base64 decode
                if let Ok(bytes) = base64_decode(data_str) {
                    if let Some(event) = Self::decode_tipped_xdr(&bytes, &rpc_event.tx_hash, ledger)
                    {
                        return Ok(Some(event));
                    }
                }
                eprintln!(
                    "  Warning: could not decode GistTipped event data: {}",
                    &data_str[..data_str.len().min(80)]
                );
                Ok(None)
            }
            "claimed" => {
                if let Ok(data) = serde_json::from_str::<ClaimedData>(data_str) {
                    return Ok(Some(VaultEvent::Claimed(ClaimedEvent {
                        txn_hash: rpc_event.tx_hash.clone(),
                        ledger,
                        recipient: data.recipient,
                        amount: data.amount,
                    })));
                }
                if let Ok(bytes) = base64_decode(data_str) {
                    if let Some(event) =
                        Self::decode_claimed_xdr(&bytes, &rpc_event.tx_hash, ledger)
                    {
                        return Ok(Some(event));
                    }
                }
                eprintln!(
                    "  Warning: could not decode TipsClaimed event data: {}",
                    &data_str[..data_str.len().min(80)]
                );
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    /// Minimal XDR decoding for GistTippedEvent.
    /// The XDR layout is:
    ///   - i32 gist_id (4 bytes, big-endian)
    ///   - Address recipient (32 bytes, ed25519 public key)
    ///   - i128 amount (16 bytes, big-endian)
    ///   - i128 fee (16 bytes, big-endian)
    ///   - i128 net (16 bytes, big-endian)
    fn decode_tipped_xdr(bytes: &[u8], txn_hash: &str, ledger: u32) -> Option<VaultEvent> {
        if bytes.len() < 84 {
            return None;
        }
        let gist_id = u64::from_be_bytes(bytes[0..8].try_into().ok()?);
        let recipient_bytes: [u8; 32] = bytes[8..40].try_into().ok()?;
        let recipient = format!("G{}", hex::encode(recipient_bytes));
        let amount = i128::from_be_bytes(bytes[40..56].try_into().ok()?);
        let fee = i128::from_be_bytes(bytes[56..72].try_into().ok()?);
        let net = i128::from_be_bytes(bytes[72..88].try_into().ok()?);

        Some(VaultEvent::Tipped(TippedEvent {
            txn_hash: txn_hash.to_string(),
            ledger,
            gist_id,
            recipient,
            amount,
            fee,
            net,
        }))
    }

    /// Minimal XDR decoding for TipsClaimedEvent.
    /// The XDR layout is:
    ///   - Address recipient (32 bytes, ed25519 public key)
    ///   - i128 amount (16 bytes, big-endian)
    fn decode_claimed_xdr(bytes: &[u8], txn_hash: &str, ledger: u32) -> Option<VaultEvent> {
        if bytes.len() < 48 {
            return None;
        }
        let recipient_bytes: [u8; 32] = bytes[0..32].try_into().ok()?;
        let recipient = format!("G{}", hex::encode(recipient_bytes));
        let amount = i128::from_be_bytes(bytes[32..48].try_into().ok()?);

        Some(VaultEvent::Claimed(ClaimedEvent {
            txn_hash: txn_hash.to_string(),
            ledger,
            recipient,
            amount,
        }))
    }
}

/// Simple base64 decoding (no-std compatible, avoids adding the base64 crate).
fn base64_decode(s: &str) -> Result<Vec<u8>, ()> {
    const TABLE: [i8; 128] = [
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
        -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, -1, 62, -1, -1,
        -1, 63, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, -1, -1, -1, -1, -1, -1, -1, 0, 1, 2, 3, 4,
        5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, -1, -1, -1,
        -1, -1, -1, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45,
        46, 47, 48, 49, 50, 51, -1, -1, -1, -1, -1,
    ];

    let mut output = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;

    for &byte in s.as_bytes() {
        if byte == b'=' {
            break;
        }
        let val = TABLE[byte as usize];
        if val < 0 {
            return Err(());
        }
        buf = (buf << 6) | (val as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buf >> bits) as u8);
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_decode() {
        let decoded = base64_decode("SGVsbG8=").expect("should decode base64");
        assert_eq!(decoded, b"Hello");
    }

    #[test]
    fn test_decode_tipped_xdr() {
        let mut bytes = vec![0u8; 88];
        // gist_id = 42
        bytes[0..8].copy_from_slice(&42u64.to_be_bytes());
        // recipient: 32 bytes of 0xAA
        for b in &mut bytes[8..40] {
            *b = 0xAA;
        }
        // amount = 1_000_000
        bytes[40..56].copy_from_slice(&1_000_000i128.to_be_bytes());
        // fee = 50_000
        bytes[56..72].copy_from_slice(&50_000i128.to_be_bytes());
        // net = 950_000
        bytes[72..88].copy_from_slice(&950_000i128.to_be_bytes());

        let event = VaultEvent::decode_tipped_xdr(&bytes, "txhash123", 1000)
            .expect("should decode tipped XDR");
        match event {
            VaultEvent::Tipped(t) => {
                assert_eq!(t.gist_id, 42);
                assert_eq!(t.amount, 1_000_000);
                assert_eq!(t.fee, 50_000);
                assert_eq!(t.net, 950_000);
                assert_eq!(t.ledger, 1000);
            }
            _ => panic!("expected Tipped event"),
        }
    }

    #[test]
    fn test_decode_claimed_xdr() {
        let mut bytes = vec![0u8; 48];
        for b in &mut bytes[0..32] {
            *b = 0xBB;
        }
        bytes[32..48].copy_from_slice(&2_500_000i128.to_be_bytes());

        let event = VaultEvent::decode_claimed_xdr(&bytes, "txhash456", 2000)
            .expect("should decode claimed XDR");
        match event {
            VaultEvent::Claimed(c) => {
                assert_eq!(c.amount, 2_500_000);
                assert_eq!(c.ledger, 2000);
            }
            _ => panic!("expected Claimed event"),
        }
    }
}
