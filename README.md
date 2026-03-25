# LEZ Private Multisig — Privacy-Preserving M-of-N Governance

A privacy-preserving M-of-N multisig governance program for the [Logos Execution Zone (LEZ)](https://github.com/logos-blockchain/lssa). Members vote on proposals using **zero-knowledge proofs** — only vote nullifiers appear on-chain, hiding voter identity.

**LP-0002 submission** — see [docs/lp-0002-writeup.md](docs/lp-0002-writeup.md) for the full technical write-up.

## How It Works

```
CreateMultisig → Propose → Approve (xM) → Execute → ChainedCall to target program
                    ▲           ▲             ▲
                    └───────────┴─────────────┘
                    ZK proof of NSK knowledge
                    (voter identity stays private)
```

1. **Create** a multisig with N member NPKs (nullifier public keys), threshold M
2. **Propose** an action — member proves identity via ZK proof, creates proposal PDA
3. **Approve** — members approve independently, each proving membership via ZK
4. **Execute** — once M approvals collected, emits a `ChainedCall` to the target program
5. **Reject** — members can reject; if rejections >= (N - M + 1), the proposal is dead

**Privacy model:** The multisig uses NPK-based membership. Members prove they know a corresponding NSK (nullifier secret key) via a RISC0 ZK proof. The NSK never leaves the client — only a domain-separated vote nullifier appears on-chain. Observers see that "someone in the member set voted" but not who.

## Project Structure

```
lez-multisig/
├── multisig_core/           — shared types, instructions, PDA derivation
├── multisig_program/        — on-chain handlers (RISC0 guest)
├── vote_circuit/            — ZK membership proof + nullifier derivation
├── multisig_client/         — client-side ZK proof generation (VoteProver)
├── cli/                     — multisig CLI (create/propose/approve/execute)
├── methods/                 — RISC0 zkVM guest build config
│   └── guest/src/bin/
│       ├── multisig.rs      — main guest entry point
│       └── vote_circuit.rs  — vote circuit guest binary
├── e2e_tests/               — integration tests against live sequencer
├── scripts/
│   ├── demo-private-multisig.sh — E2E demo script
│   └── build-guest.sh       — Docker wrapper for reproducible builds
├── docs/
│   └── lp-0002-writeup.md   — LP-0002 submission write-up
└── SPEC.md                  — technical specification
```

## Quick Start

### Prerequisites

- Rust nightly (edition 2024)
- [RISC0 toolchain](https://dev.risczero.com/api/zkvm/install): `curl -L https://risczero.com/install | bash && rzup install`
- Docker (for reproducible guest builds)
- Clone of [lssa](https://github.com/logos-blockchain/lssa) (for sequencer + wallet + token program)

### 1. Build

```bash
cd lez-multisig

# Dev mode (fast, ~30s) — proofs are fake but functional
RISC0_DEV_MODE=1 cargo build -p multisig-methods -p multisig-cli --release

# Production (reproducible Docker build, ~15-20 min)
cargo risczero build --manifest-path methods/guest/Cargo.toml
```

### 2. Run Unit Tests

```bash
cargo test --lib
```

### 3. Run the E2E Demo

```bash
# Start localnet (in a separate terminal)
cd /tmp/spel-privacy-test && logos-scaffold localnet start

# Run the demo
./scripts/demo-private-multisig.sh
```

The demo creates a 2-of-3 multisig, proposes an action, collects 2 ZK-verified approvals, and executes the proposal.

### 4. Run E2E Tests (Full Token Transfer)

```bash
export MULTISIG_PROGRAM=$(pwd)/target/riscv-guest/.../multisig.bin
export TOKEN_PROGRAM=~/lssa/artifacts/program_methods/token.bin
cargo test -p lez-multisig-e2e -- --nocapture
```

## CLI Usage

```bash
# Derive NPK from NSK
multisig derive-npk --nsk <64-hex-chars>

# Create 2-of-3 multisig
multisig create --threshold 2 --members <npk1> <npk2> <npk3>

# Propose (generates ZK proof)
multisig propose --multisig <create-key> --nsk <nsk-hex> \
    --members <npk1> <npk2> <npk3> \
    --target-program <program-id-hex> --target-account-count 0 \
    --proposal-index 1

# Approve (generates ZK proof)
multisig approve --multisig <create-key> --proposal 1 \
    --nsk <nsk-hex> --members <npk1> <npk2> <npk3>

# Execute (after threshold approvals)
multisig execute --multisig <create-key> --proposal 1 \
    --nsk <nsk-hex> --members <npk1> <npk2> <npk3>
```

## On-Chain State

See [SPEC.md](SPEC.md) for full details.

| Account | PDA Seed | Purpose |
|---------|----------|---------|
| Multisig State | `"multisig_state__" XOR create_key` | Config: member NPKs, threshold, tx counter |
| Proposal | `"multisig_prop___" XOR create_key XOR index` | Single proposal: action + vote nullifiers |
| Vault | `"multisig_vault__" XOR create_key` | Holds assets controlled by multisig |

## Known Limitations

- IMAGE_ID is a placeholder `[0; 8]` — must be updated for production
- Member list (NPKs) is public on-chain
- No member rotation instructions yet (`AddMember`, `RemoveMember`, `ChangeThreshold`)
- No `CloseProposal` instruction (executed/rejected proposals stay on-chain)
- PDA derivation uses legacy XOR formula (not NSSA standard SHA256-prefix)

## References

- [LP-0002 Write-up](docs/lp-0002-writeup.md) — cryptographic design, security analysis, benchmarks
- [Technical Specification (SPEC.md)](SPEC.md) — accounts, PDA derivation, instruction set
- [LSSA Repository](https://github.com/logos-blockchain/lssa) — LEZ framework
- [Squads Protocol v4](https://squads.so/) — design inspiration

## License

MIT — see [LICENSE](LICENSE)
