# Contract Events

All TheGist contracts emit Soroban events that the off-chain indexer (TheGist-API) consumes.

## Topic convention

Every event follows:

```
topics = [Symbol(contract_name), Symbol(event_name)]
data   = <typed struct>
```

The indexer filters by the two topic symbols. Data is a `contracttype` struct that can be decoded from the XDR payload.

---

## GistRegistry

### GistPosted

Emitted every time a gist is successfully written on-chain.

| Field | Type | Description |
|-------|------|-------------|
| `gist_id` | `u64` | Auto-assigned on-chain ID |
| `author` | `Address` | Stellar address of the signer |
| `timestamp` | `u64` | Ledger timestamp at submission |

```
topics = [symbol("gist"), symbol("posted")]
data   = GistPostedEvent { gist_id, author, timestamp }
```

Example payload:
```json
{ "gist_id": 42, "author": "GABC...XYZ", "timestamp": 1719000000 }
```

---

### GistExpired

Emitted when a gist is manually expired (by author or admin).

| Field | Type | Description |
|-------|------|-------------|
| `gist_id` | `u64` | ID of the expired gist |
| `expired_by` | `Address` | Address that triggered expiry |

```
topics = [symbol("gist"), symbol("expired")]
data   = GistExpiredEvent { gist_id, expired_by }
```

---

### ContractUpgraded

Emitted when the contract is upgraded to a new version.

| Field | Type | Description |
|-------|------|-------------|
| `old_version` | `u32` | Previous contract version |
| `new_version` | `u32` | New contract version |

```
topics = [symbol("contract"), symbol("upgraded")]
data   = ContractUpgradedEvent { old_version, new_version }
```

---

## GistVault

### GistTipped

Emitted when a user tips a gist author.

| Field | Type | Description |
|-------|------|-------------|
| `gist_id` | `u64` | ID of the tipped gist |
| `recipient` | `Address` | Author receiving the tip |
| `amount` | `i128` | Amount tipped (in stroops) |

```
topics = [symbol("vault"), symbol("tipped")]
data   = GistTippedEvent { gist_id, recipient, amount }
```

---

### TipsClaimed

Emitted when an author withdraws their accumulated tips.

| Field | Type | Description |
|-------|------|-------------|
| `recipient` | `Address` | Author claiming tips |
| `amount` | `i128` | Total amount claimed (in stroops) |

```
topics = [symbol("vault"), symbol("claimed")]
data   = TipsClaimedEvent { recipient, amount }
```

---

## LocationVerifier

### PrefixAdded

Emitted when an allowed geohash prefix is added.

| Field | Type | Description |
|-------|------|-------------|
| `prefix` | `String` | Geohash prefix that was added |

```
topics = [symbol("location"), symbol("pfx_add")]
data   = PrefixAddedEvent { prefix }
```

---

### PrefixRemoved

Emitted when an allowed geohash prefix is removed.

| Field | Type | Description |
|-------|------|-------------|
| `prefix` | `String` | Geohash prefix that was removed |

```
topics = [symbol("location"), symbol("pfx_rm")]
data   = PrefixRemovedEvent { prefix }
```

---

## Indexer filtering example

```typescript
// Subscribe to all GistPosted events
const events = await rpc.getEvents({
  filters: [{ topics: [["gist", "posted"]] }],
});
```
