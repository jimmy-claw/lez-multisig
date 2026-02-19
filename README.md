# LEZ Multisig — M-of-N Threshold Signatures

An M-of-N multisig program for the [Logos Execution Zone (LEZ)](https://github.com/logos-blockchain/lssa). Multiple signers must approve transfers before they execute — no single key can drain the funds.

📄 **[FURPS Specification](docs/FURPS.md)** — functional requirements, usability, reliability, performance, security constraints.

## How It Works

- **Create** a multisig with N members and threshold M
- **Propose** a transfer — creates a proposal that requires M approvals
- **Sign** a proposal — each signer approves independently
- **Execute** — once M signatures are collected, the transfer goes through
- **Manage** members and threshold (add/remove members, change threshold) — also requires M signatures
- State lives in a **PDA** (Program Derived Account) — only the multisig program controls it

## Project Structure

```
lez-multisig/
├── multisig_core/           — shared types, instructions, PDA helpers
├── multisig_program/        — on-chain handlers
│   └── src/
│       ├── create_multisig.rs
│       ├── execute.rs
│       ├── add_member.rs
│       ├── remove_member.rs
│       └── change_threshold.rs
├── cli/                     — CLI module (imported by lez-wallet)
│   └── multisig.rs          — clap subcommands for multisig ops
├── methods/                 — risc0 zkVM guest build
│   └── guest/src/bin/multisig.rs
├── examples/
│   └── program_deployment/  — standalone deployment CLI
└── docs/
    └── FURPS.md             — requirements specification
```

## Quick Start

### Prerequisites

- Rust nightly (edition 2024)
- [Risc0 toolchain](https://dev.risczero.com/api/zkvm/install): `curl -L https://risczero.com/install | bash && rzup install`
- A running LSSA sequencer

### Build

```bash
# Check core logic
cargo check -p multisig_core -p multisig_program

# Build the zkVM guest (produces the on-chain binary)
cargo risczero build --manifest-path methods/guest/Cargo.toml

# Build the deployment CLI
cargo build --bin multisig -p program_deployment
```

### Deploy

```bash
# Start the sequencer (from lssa repo)
cd /path/to/lssa/sequencer_runner
RUST_LOG=info cargo run $(pwd)/configs/debug

# Deploy the multisig program
wallet deploy-program target/riscv32im-risc0-zkvm-elf/docker/treasury.bin
```

## CLI Usage

The multisig CLI is designed as a subcommand module for `lez-wallet`:

```bash
# Create a 2-of-3 multisig
lez-wallet multisig create --threshold 2 --member <PK1> --member <PK2> --member <PK3>

# View multisig info
lez-wallet multisig info --account <MULTISIG_ID>

# Propose a transfer
lez-wallet multisig propose --multisig <ID> --to <RECIPIENT> --amount 100

# Sign a proposal
lez-wallet multisig sign --proposal <FILE> --output <SIGNED_FILE>

# Execute (after M signatures collected)
lez-wallet multisig execute --proposal <FILE>

# Manage members
lez-wallet multisig add-member --multisig <ID> --member <NEW_PK>
lez-wallet multisig remove-member --multisig <ID> --member <PK>
lez-wallet multisig change-threshold --multisig <ID> --threshold 3
```

## Tests

```bash
cargo test -p multisig_program
```

18 unit tests covering creation, execution, member management, threshold changes, and edge cases (duplicate members, threshold bounds, replay protection via nonce).

## References

- [LSSA Repository](https://github.com/logos-blockchain/lssa)
- [FURPS Specification](docs/FURPS.md)
