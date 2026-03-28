# LP-0002 Private Multisig — Verification Guide

**For Václav and Lambda Prize reviewers.**

## What to Verify

LP-0002 adds ZK-based private voting to the LEZ multisig. Three claims to verify:

1. **Voter anonymity**: On-chain records contain nullifiers, not voter identities
2. **Double-vote prevention**: Same member can't vote twice on the same proposal
3. **Proof composition**: Vote circuit receipt is verified on-chain via `env::verify`

---

## Quick Verification (~5 min, dev mode)

### Prerequisites
- SSH access to crib: `ssh jimmy@192.168.0.152`
- All binaries pre-built on the `jimmy/private-multisig-v2` branch

### Run

```bash
# 1. Fresh sequencer
killall sequencer_runner 2>/dev/null; sleep 2
rm -rf ~/lp0002-localnet/rocksdb
cd ~/lp0002-localnet
RISC0_DEV_MODE=1 nohup ~/.cache/logos-scaffold/repos/lssa/target/release/sequencer_runner \
  ~/.cache/logos-scaffold/repos/lssa/sequencer_runner/configs/debug > ~/sequencer.log 2>&1 &
sleep 4

# 2. Run demo
cd ~/lez-multisig
NSSA_WALLET_HOME_DIR=/tmp/multisig-demo-wallet \
RISC0_DEV_MODE=1 \
SPEL_CLI=$HOME/spel/target/release/spel \
SPEL_GUEST_ELF=target/riscv-guest/multisig-methods/multisig-guest/riscv32im-risc0-zkvm-elf/release/vote-circuit.bin \
bash scripts/demo-private-multisig.sh
```

### Expected Output

All 9 steps should print "Transaction confirmed":

| Step | Action | What Happens |
|------|--------|--------------|
| 1 | Deploy programs | Multisig + token deployed |
| 2 | Create members | 3 private accounts created |
| 3 | Register on-chain | auth-transfer init (Merkle commitments) |
| 4 | Create multisig | 2-of-3 with NPKs |
| 5 | Create token | Mint + fund vault with 500 |
| 6 | Propose (MEMBER1) | ZK proof → privacy TX → confirmed |
| 7 | Approve (MEMBER2) | ZK proof → privacy TX → confirmed |
| 8 | Approve (MEMBER3) | ZK proof → privacy TX → confirmed |
| 9 | Execute | ChainedCall token transfer → confirmed |

### What to Look For in the Output

**Proof generation** — Steps 6-8 should show:
```
Running pre_tx hook (elf: vote_circuit.bin)...
  Proving...
  nullifier: <32-byte hex>
  vote_receipt: <receipt bytes>
  circuit receipt claim digest: <hash>
✅ pre_tx hook complete
```

**Privacy TX** — Steps 6-8 should show:
```
Privacy-preserving transaction submitted!
  tx_hash: <hash>
  Waiting for confirmation...
Transaction confirmed
```

**Privacy trace** — End of script prints what observer can/can't know.

---

## Manual Verification Steps

### 1. Verify voter anonymity

After running the demo, check the sequencer log for the propose/approve TXs:

```bash
# Show all successful TXs (skip deploy errors)
strings ~/sequencer.log | grep -v "ERROR"
```

The sequencer processes privacy-preserving TXs. It sees:
- ✅ Encrypted account state changes
- ✅ Nullifiers (unique per vote)
- ❌ No voter account IDs
- ❌ No link between nullifier and member

### 2. Verify double-vote prevention

The proposer (MEMBER1) auto-approves during propose. If you modify the demo to have MEMBER1 approve again:

```bash
# In the demo script, change Step 7 --caller to $MEMBER1
# Expected: sequencer rejects with "Nullifier already seen"
```

Check: `strings ~/sequencer.log | grep "Nullifier already seen"`

This was verified in demo run 6 (log: `~/lp0002-demo-run6.log`).

### 3. Verify env::verify (proof composition)

The vote circuit receipt is verified inside the on-chain multisig guest. To confirm this is real:

```bash
# Check the guest binary includes env::verify call
cd ~/lez-multisig
grep -r "env::verify" multisig_program/src/

# Should show env::verify in approve.rs, propose.rs, reject.rs
```

The IMAGE_ID is pinned at build time:
```bash
grep "VOTE_CIRCUIT_IMAGE_ID" multisig_core/src/lib.rs
```

### 4. Verify the 3-repo changes

Each repo has a feature branch:

```bash
# lssa — extra_assumptions parameter
cd ~/lssa && git log --oneline jimmy/wallet-zk-helpers -5

# spel — pre_tx_hook + privacy routing  
cd ~/spel && git log --oneline jimmy/pre-tx-hook -5

# lez-multisig — vote circuit + handlers
cd ~/lez-multisig && git log --oneline jimmy/private-multisig-v2 -10
```

---

## Code Review Checklist

### Vote Circuit (`multisig_program/src/vote_circuit/`)
- [ ] Guest reads NSK, members, proposal_index, domain
- [ ] Derives NPK from NSK, checks membership
- [ ] Computes domain-separated nullifier (approve ≠ reject ≠ propose)
- [ ] Commits nullifier + proposal_index to journal

### On-Chain Handlers (`multisig_program/src/approve.rs`, `propose.rs`, `reject.rs`)
- [ ] Calls `env::verify(VOTE_CIRCUIT_IMAGE_ID, &vote_receipt)`
- [ ] Extracts nullifier from vote receipt
- [ ] Checks nullifier not already used
- [ ] Records nullifier (not voter identity)
- [ ] `#[account(signer)] caller` as private account

### SPEL Integration (`~/spel/spel-cli/src/tx.rs`)
- [ ] `run_pre_tx_hook` returns `Option<Receipt>`
- [ ] Receipt passed as assumption: `if has_private || pre_tx_receipt.is_some()`
- [ ] Extra assumptions threaded to `send_privacy_preserving_tx`

### LSSA Changes (`~/lssa/wallet/src/lib.rs`, `~/lssa/nssa/src/...`)
- [ ] `execute_and_prove_program` accepts `extra_assumptions: Vec<Receipt>`
- [ ] `send_privacy_preserving_tx` accepts and passes assumptions
- [ ] Assumptions added to env_builder via `add_assumption()`

---

## Known Limitations

1. **Dev mode proofs**: Demo uses `RISC0_DEV_MODE=1` for speed. Real proofs take ~47 min per vote on the i3-7100T. Production would use GPU proving.
2. **Image ID not reproducible**: Built locally, not via Docker reproducible build. Would need `cargo risczero build` for verifiable image ID.
3. **NPK set is public**: Member NPKs visible at multisig creation. Passive observer can't link votes, but key compromise enables attribution.
4. **No batch execute yet**: Each approval is a separate TX. A batch_execute coordinator flow is planned.

---

## File Locations on Crib

| What | Path |
|------|------|
| Multisig repo | `~/lez-multisig` (branch `jimmy/private-multisig-v2`) |
| SPEL repo | `~/spel` (branch `jimmy/pre-tx-hook`) |
| LSSA repo | `~/lssa` (branch `jimmy/wallet-zk-helpers`) |
| Demo script | `~/lez-multisig/scripts/demo-private-multisig.sh` |
| Vote circuit guest | `~/lez-multisig/multisig_program/src/vote_circuit/` |
| Spel CLI binary | `~/spel/target/release/spel` |
| Sequencer binary | `~/.cache/logos-scaffold/repos/lssa/target/release/sequencer_runner` |
| Demo log (passing) | `~/lp0002-demo-run7.log` |
| Sequencer log | `~/sequencer.log` |
| Test identity (isolated test) | `~/test-identity/` |

---

*Last updated: 2026-03-28. All verification done on crib (192.168.0.152), i3-7100T.*
