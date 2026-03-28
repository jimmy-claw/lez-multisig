# LP-0002 Private Multisig — Demo Instructions

## Prerequisites

All binaries are pre-built on crib. You need:
- SSH access to crib: `ssh jimmy@192.168.0.152`
- Repos: `~/lez-multisig` (branch `jimmy/private-multisig-v2`), `~/spel`, `~/lssa`

## Quick Run (automated)

```bash
# 1. Start fresh sequencer
killall sequencer_runner 2>/dev/null; sleep 2
rm -rf ~/lp0002-localnet/rocksdb
cd ~/lp0002-localnet
RISC0_DEV_MODE=1 nohup ~/.cache/logos-scaffold/repos/lssa/target/release/sequencer_runner \
  ~/.cache/logos-scaffold/repos/lssa/sequencer_runner/configs/debug > ~/sequencer.log 2>&1 &
sleep 4
curl -so /dev/null -w '%{http_code}' http://127.0.0.1:3040/  # should print 405

# 2. Run the demo (~5 min in dev mode)
cd ~/lez-multisig
NSSA_WALLET_HOME_DIR=/tmp/multisig-demo-wallet \
RISC0_DEV_MODE=1 \
SPEL_CLI=$HOME/spel/target/release/spel \
SPEL_GUEST_ELF=target/riscv-guest/multisig-methods/multisig-guest/riscv32im-risc0-zkvm-elf/release/vote-circuit.bin \
bash scripts/demo-private-multisig.sh
```

## What the Demo Does (9 steps)

1. **Build + deploy** multisig and token programs
2. **Create 3 private member accounts** (wallet)
3. **Register on-chain** via auth-transfer init (creates Merkle commitments)
4. **Create 2-of-3 multisig** with member NPKs
5. **Create token**, fund vault with 500 tokens
6. **Propose** 200-token transfer (MEMBER1, ZK proof via `#[pre_tx_hook]`)
7. **Approve** (MEMBER2, anonymous ZK vote)
8. **Approve** (MEMBER3, anonymous ZK vote — threshold met)
9. **Execute** — ChainedCall transfers 200 tokens vault → recipient

## Key Environment Variables

| Variable | Purpose |
|----------|---------|
| `RISC0_DEV_MODE=1` | Use fast dev proofs (both client AND sequencer) |
| `NSSA_WALLET_HOME_DIR` | Wallet storage directory (fresh per run) |
| `SPEL_CLI` | Path to spel CLI binary |
| `SPEL_GUEST_ELF` | Path to vote_circuit.bin for pre_tx hook |

## Rebuilding (if code changed)

```bash
# Rebuild guest binaries (after changing multisig_program code)
cd ~/lez-multisig
rm -rf target/riscv-guest
RISC0_DEV_MODE=1 cargo build --release  # ~2 min

# Rebuild spel CLI (after changing spel code)
cd ~/spel
cargo build --release -p spel  # ~30s

# Rebuild sequencer (after changing lssa code)
rsync -av ~/lssa/nssa/ ~/.cache/logos-scaffold/repos/lssa/nssa/
rsync -av ~/lssa/wallet/ ~/.cache/logos-scaffold/repos/lssa/wallet/
rsync -av ~/lssa/common/ ~/.cache/logos-scaffold/repos/lssa/common/
cd ~/.cache/logos-scaffold/repos/lssa
touch sequencer_runner/src/main.rs
cargo build --release --features standalone -p sequencer_runner  # ~30s
```

## Troubleshooting

- **TX NOT confirmed**: Check `strings ~/sequencer.log | grep ERROR | tail -5`
- **InvalidPrivacyPreservingProof**: Sequencer not started with `RISC0_DEV_MODE=1`
- **Empty commitments**: Missing auth-transfer init step
- **Nullifier already seen**: Member already voted (double-vote prevention working correctly)
- **r0vm using all CPU**: That's the RISC0 prover — normal, just wait
