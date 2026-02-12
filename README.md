# Treasury Program — PDA (Program Derived Accounts) Example

A demonstration program for the [Logos Execution Zone (LEZ)](https://github.com/logos-blockchain/lssa) that shows how programs can own and control accounts through **Program Derived Accounts (PDAs)**, and compose with other programs through **chained calls**.

## What Does This Program Do?

The Treasury program acts as an on-chain vault manager. It can:

1. **Create Vaults** — deploy a new token and mint initial supply into a treasury-controlled vault
2. **Send** — transfer tokens from a vault to any recipient
3. **Deposit** — receive tokens from external senders into a vault

All vault accounts are **PDAs** — accounts whose authority is derived from the Treasury program itself, not from any external key. This means only the Treasury program can authorize actions on its vaults.

## Understanding PDAs

### What is a PDA?

A **Program Derived Account (PDA)** is an account whose ID (address) is deterministically computed from:
- A **program ID** (which program "owns" the PDA)
- One or more **seeds** (arbitrary bytes that make each PDA unique)

```
PDA Account ID = hash(program_id || seed)
```

PDAs are special because:
- **No private key** corresponds to them — nobody can sign for them externally
- **Only the deriving program** can authorize operations on them
- **Deterministic** — anyone can recompute the address given the program ID and seeds

### Authority vs Ownership

In NSSA/LEZ, there's an important distinction:

| Concept | Meaning |
|---------|---------|
| **Ownership** | Which program's code runs when the account is accessed |
| **Authority** | Who can authorize writes/transfers from the account |

For a PDA:
- The **program** that derived it has authority
- The program proves authority by providing the **PDA seeds** in a chained call
- The runtime verifies: `hash(program_id, seeds) == account_id`

### PDA Derivation in This Program

```
┌─────────────────────────────────────────────────────┐
│                   Treasury Program                   │
│                  (treasury_program_id)               │
└──────────┬──────────────────────┬───────────────────┘
           │                      │
           │ seed: "treasury_state"    seed: token_definition_id
           │                      │
           ▼                      ▼
    ┌──────────────┐      ┌──────────────────┐
    │ Treasury     │      │ Vault Holding    │
    │ State PDA    │      │ PDA              │
    │              │      │                  │
    │ hash(        │      │ hash(            │
    │  program_id, │      │  program_id,     │
    │  "treasury_  │      │  token_def_id    │
    │   state"     │      │ )                │
    │ )            │      │                  │
    └──────────────┘      └──────────────────┘
```

- **Treasury State PDA**: `hash(treasury_program_id, "treasury_state")` — stores vault count and metadata
- **Vault Holding PDA**: `hash(treasury_program_id, token_definition_id)` — one per token, holds the treasury's balance

## Project Structure

```
lssa-treasury/
├── Cargo.toml                    — workspace definition
├── README.md                     — this file
├── treasury_core/                — shared types (no runtime dependency)
│   └── src/lib.rs                — Instruction enum, TreasuryState, PDA helpers
├── treasury_program/             — on-chain program logic
│   └── src/
│       ├── lib.rs                — dispatch entry point
│       ├── create_vault.rs       — CreateVault handler
│       ├── send.rs               — Send handler
│       └── receive.rs            — Deposit handler
├── methods/                      — risc0 build infrastructure
│   ├── build.rs                  — embeds guest ELF
│   └── guest/
│       └── src/bin/treasury.rs   — zkVM guest binary entry point
└── examples/
    └── program_deployment/       — off-chain examples
        └── src/bin/
            ├── deploy_and_create_vault.rs
            └── send_from_vault.rs
```

## Code Walkthrough

### 1. PDA Derivation (`treasury_core/src/lib.rs`)

The core crate provides deterministic PDA computation:

```rust
pub fn compute_treasury_state_pda(treasury_program_id: &ProgramId) -> AccountId {
    AccountId::compute_pda(treasury_program_id, b"treasury_state")
}

pub fn compute_vault_holding_pda(
    treasury_program_id: &ProgramId,
    token_definition_id: &AccountId,
) -> AccountId {
    AccountId::compute_pda(treasury_program_id, token_definition_id.as_bytes())
}
```

These functions are used both on-chain (inside the zkVM) and off-chain (in deployment scripts) to derive the same addresses.

### 2. CreateVault (`treasury_program/src/create_vault.rs`)

This instruction demonstrates three key patterns:

**a) First-time PDA claiming:**
```rust
treasury_state_account.post_state = AccountPostState::new_claimed_if_default();
```
This claims the PDA if it hasn't been claimed yet (idempotent).

**b) Authorizing a PDA for cross-program use:**
```rust
vault_holding.is_authorized = true;
```
Setting `is_authorized = true` tells the runtime that this program is granting its authority over this PDA to the next program in the chain.

**c) Building a chained call with PDA seeds:**
```rust
let chained_call = ChainedCall::new(
    token_program_id.clone(),
    token_ix_data,
    chained_accounts,
)
.with_pda_seeds(vec![vault_holding_pda_seed(&token_definition_id)]);
```
The `.with_pda_seeds()` provides the seeds so the runtime can verify that the Treasury program actually owns the vault PDA.

### 3. Send (`treasury_program/src/send.rs`)

Demonstrates transferring from a PDA:

```rust
// Authorize the vault (treasury is the authority)
vault_holding.is_authorized = true;

// Chain to Token::Transfer
let chained_call = ChainedCall::new(...)
    .with_pda_seeds(vec![vault_holding_pda_seed(&token_definition_id)]);
```

The flow:
1. Treasury program marks `vault_holding.is_authorized = true`
2. Provides PDA seeds to prove ownership
3. Runtime verifies `hash(treasury_program_id, seed) == vault_holding.id`
4. Token program sees an authorized source account and executes the transfer

### 4. Deposit (`treasury_program/src/receive.rs`)

Deposits are simpler because the vault PDA is the *receiver*:

```rust
// No is_authorized needed — vault is receiving, not sending
let chained_call = ChainedCall::new(
    token_program_id.clone(),
    token_ix_data,
    chained_accounts,
);
// No .with_pda_seeds() needed — the sender is authorized by the caller
```

The sender's authorization comes from the original transaction signer, not from the Treasury program.

### 5. Guest Binary (`methods/guest/src/bin/treasury.rs`)

The guest binary is minimal — it just bridges the zkVM environment to the program logic:

```rust
fn main() {
    let mut program_input = read_nssa_inputs();
    let (updated_accounts, chained_call) = treasury_program::process(
        &program_input.program_id,
        &mut program_input.accounts,
        &program_input.input_data,
    );
    match chained_call {
        Some(call) => write_nssa_outputs_with_chained_call(updated_accounts, call),
        None => write_nssa_outputs(updated_accounts),
    }
}
```

## Build

### Prerequisites

- Rust toolchain (nightly recommended for risc0)
- [risc0 toolchain](https://dev.risczero.com/api/zkvm/install)

### Build the guest binary

```bash
cargo build --release
```

This invokes `risc0_build::embed_methods()` which compiles the guest binary for the RISC-V target and embeds it in the `treasury-methods` crate.

### Run the examples

```bash
# Show how deployment + CreateVault works
cargo run --bin deploy_and_create_vault

# Show how Send works
cargo run --bin send_from_vault
```

## Chained Call Flow

Here's the full execution flow for a `CreateVault` call:

```
User / Off-chain
    │
    │  ProgramInput { program_id: treasury, accounts: [...], data: CreateVault{...} }
    │
    ▼
┌────────────────────────────────────┐
│  NSSA Runtime (executes in zkVM)   │
│                                    │
│  1. Load Treasury guest ELF        │
│  2. Execute treasury.rs::main()    │
│     └─ treasury_program::process() │
│        └─ create_vault::handle()   │
│           ├─ Update TreasuryState  │
│           ├─ vault.is_authorized=true │
│           └─ Return ChainedCall    │
│                                    │
│  3. Verify PDA seeds:              │
│     hash(treasury_id, seed)        │
│       == vault_holding.id ✓        │
│                                    │
│  4. Execute ChainedCall:           │
│     Token::NewFungibleDefinition   │
│     ├─ Create token definition     │
│     └─ Mint to vault_holding       │
│                                    │
│  5. Commit all state changes       │
└────────────────────────────────────┘
```

## Key Concepts Summary

| Pattern | Where Used | Purpose |
|---------|-----------|---------|
| `AccountId::compute_pda()` | `treasury_core` | Deterministic address derivation |
| `is_authorized = true` | `create_vault`, `send` | Grant PDA authority to chained program |
| `.with_pda_seeds()` | `create_vault`, `send` | Prove PDA ownership to the runtime |
| `AccountPostState::new_claimed()` | `create_vault` | First-time account creation |
| `AccountPostState::new_claimed_if_default()` | `create_vault`, `send` | Idempotent account claiming |
| `ChainedCall::new()` | All handlers | Cross-program invocation |
| `write_nssa_outputs_with_chained_call()` | Guest binary | Return results + chained call |

## References

- [LSSA Repository](https://github.com/logos-blockchain/lssa) — full framework source
- `programs/amm/` — AMM program (advanced PDA + chained call patterns)
- `programs/token/` — Token program (the program we chain to)
- `nssa/core/src/program.rs` — core types (`ProgramInput`, `ChainedCall`, `PdaSeed`, etc.)
- `examples/program_deployment/` — simpler hello-world deployment examples
