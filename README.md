# Treasury Program — PDA Example for LSSA/LEZ

A standalone demonstration of **Program Derived Accounts (PDAs)** for the [Logos Execution Zone (LEZ)](https://github.com/logos-blockchain/lssa). This program does **not** depend on the Token program — it manages its own internal ledger of vaults and balances.

## What Does This Demonstrate?

- **PDA derivation** — deterministic account IDs from program ID + seed
- **Account claiming** — first-time initialization of PDA accounts
- **State management** — reading/writing account data in the zkVM

## Instructions

| Instruction | Description | Accounts |
|-------------|-------------|----------|
| `Init` | Initialize treasury state | treasury_state |
| `CreateVault` | Create a new vault | treasury_state, vault |
| `Deposit` | Add funds to a vault | treasury_state, vault |
| `Withdraw` | Remove funds from a vault | treasury_state, vault |
| `Transfer` | Move funds between vaults | treasury_state, from_vault, to_vault |

## Build

```bash
# Check it compiles
cargo check -p treasury_core -p treasury_program

# Build the guest binary (needs risc0 toolchain)
cargo risczero build --manifest-path methods/guest/Cargo.toml
```

## Run Examples

```bash
cd examples/program_deployment

# Initialize treasury
cargo run --bin treasury_examples ../target/riscv32im-risc0-zkvm-elf/docker/treasury.bin init

# Create a vault named "savings"
cargo run --bin treasury_examples ../target/riscv32im-risc0-zkvm-elf/docker/treasury.bin create_vault savings

# Deposit 1000 into savings
cargo run --bin treasury_examples ../target/riscv32im-risc0-zkvm-elf/docker/treasury.bin deposit savings 1000

# Create another vault and transfer
cargo run --bin treasury_examples ../target/riscv32im-risc0-zkvm-elf/docker/treasury.bin create_vault checking
cargo run --bin treasury_examples ../target/riscv32im-risc0-zkvm-elf/docker/treasury.bin transfer savings checking 500
```

## PDA Derivation

All PDA IDs are computed automatically from the program ID and vault name:

```
treasury_state PDA = hash(treasury_program_id, "treasury_state")
vault PDA          = hash(treasury_program_id, "vault" + vault_name)
```

The `treasury_core` crate provides helper functions:
- `compute_treasury_state_pda(&program_id)` → AccountId
- `compute_vault_pda(&program_id, "savings")` → AccountId
