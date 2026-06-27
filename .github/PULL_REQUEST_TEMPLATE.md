## Description

<!-- What does this PR do? Link the issue: Closes #XX -->

## Type of Change

- [ ] Bug fix
- [ ] New feature / contract logic
- [ ] Refactor (no behaviour change)
- [ ] Documentation only

---

## Security Checklist

_Required for any PR that touches contract source (`src/`). Check every item or explain why it doesn't apply._

### Authorization
- [ ] Every state-changing function calls `require_auth()` on the appropriate signer
- [ ] No function can be invoked by an unauthorized party (reviewed against `docs/SECURITY.md`)
- [ ] Admin vs user privilege separation is preserved

### Arithmetic
- [ ] No integer arithmetic without checked operations (`checked_add`, `checked_sub`, `checked_mul`)
- [ ] No unchecked casts between integer types

### Error Handling
- [ ] `panic!` messages contain no sensitive data (no addresses, private keys, or internal state)
- [ ] All `expect()` strings are safe to surface publicly

### State & Storage
- [ ] Storage keys are unique and do not collide with keys used by other contract functions
- [ ] Every state change that should be auditable emits an event
- [ ] Temporary storage TTLs are set and extended correctly

### GistVault-Specific (if touched)
- [ ] Tip amounts use `i128` with checked arithmetic; overflow is impossible
- [ ] Withdrawal zeroes the balance in storage **before** executing the token transfer
- [ ] After each deposit/withdrawal, `contract_balance == Σ pending_balances` holds

### Testing
- [ ] All new or modified functions have unit tests in `tests/`
- [ ] Tests cover the failure path (unauthorized caller, invalid input)
- [ ] `cargo test` passes locally

---

## Notes for Reviewers

<!-- Anything reviewers should pay special attention to, or known limitations -->
