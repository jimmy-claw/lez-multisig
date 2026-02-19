# LEZ Multisig — M-of-N On-Chain Proposals

An M-of-N multisig program for the [Logos Execution Zone (LEZ)](https://github.com/logos-blockchain/lssa). Inspired by [Squads Protocol v4](https://squads.so/) — proposals live on-chain and signers approve asynchronously. No offline coordination needed.

📄 **[FURPS Specification](docs/FURPS.md)** — functional requirements, usability, reliability, performance, security constraints.

## How It Works

1. **Create** a multisig with N members, threshold M, and a unique `create_key`
2. **Propose** an action (transfer, add/remove member, change threshold) — auto-approves the proposer
3. **Approve** — other members approve independently, each in their own transaction
4. **Execute** — once M approvals are collected, anyone can execute
5. **Reject** — members can reject; if rejections ≥ (N - M + 1), the proposal is dead

Each multisig gets a unique **PDA** (Program Derived Address) derived from `create_key`, allowing multiple multisigs per program deployment.

## Project Structure

```
lez-multisig/
├── multisig_core/           — shared types, instructions, PDA helpers
│   └── src/lib.rs           — MultisigState, Proposal, ProposalAction, PDA derivation
├── multisig_program/        — on-chain handlers (risc0 guest)
│   └── src/
│       ├── lib.rs           — instruction dispatch
│       ├── create_multisig.rs
│       ├── propose.rs
│       ├── approve.rs
│       ├── reject.rs
│       └── execute.rs
├── cli/                     — standalone multisig CLI binary
│   └── src/bin/
│       ├── multisig.rs      — CLI entry point
│       └── proposal.rs      — proposal helpers
├── e2e_tests/               — integration tests against live sequencer
│   └── tests/e2e_multisig.rs
├── methods/                 — risc0 zkVM guest build
│   └── guest/src/bin/multisig.rs
└── docs/
    └── FURPS.md             — requirements specification
```

## Quick Start

### Prerequisites

- Rust nightly (edition 2024)
- [Risc0 toolchain](https://dev.risczero.com/api/zkvm/install): `curl -L https://risczero.com/install | bash && rzup install`
- Docker (for reproducible guest builds)
- A running LSSA sequencer

### Build

```bash
# Check core logic + run unit tests (14 tests)
cargo test -p multisig_core -p multisig_program

# Build the zkVM guest (produces the on-chain binary)
cargo risczero build --manifest-path methods/guest/Cargo.toml
# Output: target/riscv32im-risc0-zkvm-elf/docker/multisig.bin

# Build the CLI
cargo build --bin multisig -p multisig-cli
```

### Deploy & Test

```bash
# Start the sequencer (from lssa repo)
cd /path/to/lssa && RUST_LOG=info cargo run --features standalone -p sequencer_runner -- sequencer_runner/configs/debug

# Run e2e tests (deploys program, creates multisig, proposes, approves, executes)
SEQUENCER_URL=http://127.0.0.1:3040 \
MULTISIG_PROGRAM=$(pwd)/target/riscv32im-risc0-zkvm-elf/docker/multisig.bin \
cargo test -p lez-multisig-e2e --test e2e_multisig -- --nocapture
```

## CLI Usage

```bash
# Create a 2-of-3 multisig (generates a random create_key)
multisig create --threshold 2 --member <ID1> --member <ID2> --member <ID3>
# Outputs: create_key (save this — needed for all subsequent commands)

# Create with a specific create_key
multisig create --threshold 2 --member <ID1> --member <ID2> --create-key <BASE58_KEY>

# Propose a transfer
multisig propose --multisig <CREATE_KEY> --action transfer --to <RECIPIENT> --amount 100

# Approve a proposal
multisig approve --multisig <CREATE_KEY> --proposal 1

# Reject a proposal
multisig reject --multisig <CREATE_KEY> --proposal 1

# Execute a fully-approved proposal
multisig execute --multisig <CREATE_KEY> --proposal 1

# Check multisig status
multisig status --multisig <CREATE_KEY>
```

Set `MULTISIG_PROGRAM` env var to override the program binary path.

## Architecture

### On-Chain State

```
MultisigState {
    create_key: [u8; 32],      // unique identifier, used for PDA derivation
    threshold: u8,              // M approvals needed
    member_count: u8,
    members: Vec<[u8; 32]>,    // member account IDs
    transaction_index: u64,     // auto-incrementing proposal counter
    proposals: Vec<Proposal>,   // active proposals stored in state
}

Proposal {
    index: u64,
    action: ProposalAction,     // Transfer | AddMember | RemoveMember | ChangeThreshold
    proposer: [u8; 32],
    approved: Vec<[u8; 32]>,
    rejected: Vec<[u8; 32]>,
    status: ProposalStatus,     // Active | Executed | Rejected | Cancelled
}
```

### PDA Derivation

Each multisig gets a unique PDA derived from `create_key`:
```
seed = XOR("multisig_state" padded to 32 bytes, create_key)
pda = NSSA_PDA(program_id, seed)  // internally SHA256
```

### Instruction Set

| Instruction | Accounts | Description |
|---|---|---|
| `CreateMultisig` | state_pda, creator | Initialize new multisig with members + threshold |
| `Propose` | state_pda, proposer | Create proposal, auto-approve proposer |
| `Approve` | state_pda, approver | Add approval to proposal |
| `Reject` | state_pda, rejecter | Add rejection to proposal |
| `Execute` | state_pda, executor | Execute fully-approved proposal |

## Tests

```bash
# Unit tests (14 tests — create, propose, approve, reject, execute, edge cases)
cargo test -p multisig_program

# E2e tests (requires running sequencer)
SEQUENCER_URL=http://127.0.0.1:3040 \
MULTISIG_PROGRAM=/path/to/multisig.bin \
cargo test -p lez-multisig-e2e -- --nocapture
```

## Known Issues

- CLI requires `logos-blockchain-circuits` transitive dependency ([#1](https://github.com/jimmy-claw/lez-multisig/issues/1))
- Proposals stored in `MultisigState` (no separate accounts) — may need pagination for many active proposals
- Transfer execution deducts balance but doesn't yet chain to token program

## References

- [LSSA Repository](https://github.com/logos-blockchain/lssa)
- [Squads Protocol v4](https://squads.so/) — inspiration for on-chain proposal model
- [FURPS Specification](docs/FURPS.md)
