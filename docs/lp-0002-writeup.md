# LP-0002: Privacy-Preserving Multisig for LEZ

## Overview

A privacy-preserving M-of-N multisig governance program for the Logos Execution Zone (LEZ). Members vote on proposals using zero-knowledge proofs — only vote nullifiers appear on-chain, hiding voter identity.

Built on RISC0 zkVM for ZK proof generation, with on-chain state management via PDAs and execution delegation via NSSA ChainedCalls.

## Cryptographic Approach

### Architecture

```
Client (NSK private)                    On-Chain (public)
─────────────────────                   ──────────────────
NSK (secret key)                        MultisigState { members: [NPK...], threshold }
  │                                     Proposal { approved: [nullifier...], status }
  ├──► VoteProver (RISC0)
  │    ├── Derive NPK from NSK
  │    ├── Set membership: NPK ∈ members?
  │    └── Compute nullifier
  │
  └──► Submit tx with:
       ├── vote_receipt (ZK proof)      → verified inside guest binary
       └── nullifier (32 bytes)         → stored in proposal.approved[]
```

### vote_circuit

The `vote_circuit` crate implements the ZK circuit that runs inside a RISC0 guest binary. It takes private + public inputs and produces a public output:

**Inputs (private + public):**
- `nsk: [u8; 32]` — voter's nullifier secret key (ZK witness, never revealed)
- `proposal_index: u64` — which proposal is being voted on
- `member_npks: Vec<NullifierPublicKey>` — the multisig's registered members (public)
- `domain: String` — action type: `"propose"`, `"approve"`, `"reject"`, `"execute"`

**Output (committed to RISC0 journal):**
- `nullifier: [u8; 32]` — domain-separated vote nullifier
- `proposal_index: u64` — echoed for on-chain verification

**Circuit logic:**
1. Derive NPK from NSK: `npk = NullifierPublicKey::from(&nsk)`
2. Set membership check: `assert!(member_npks.contains(&npk))`
3. Compute nullifier: `SHA256("/LEZ/v1/multisig/vote/" || domain || 0x00 || nsk || proposal_index_le)`

### NSK to NPK Derivation

Uses `nssa_core::NullifierPublicKey::from(&nsk)`, which derives the public key from the secret key via a one-way hash function. This is the same derivation used by NSSA's privacy system, ensuring compatibility with the existing key infrastructure.

The derivation is one-way: knowing the NPK does not reveal the NSK.

### Set Membership Proof

The circuit checks `member_npks.contains(&npk)` — a simple linear scan inside the ZK guest. This is a O(N) operation over the member list, acceptable for our 10-member cap.

The proof demonstrates "I know an NSK such that `NPK_from(NSK) ∈ members`" without revealing which member is voting.

## Nullifier Design

### Domain Separation

Vote nullifiers use a tagged hash scheme:

```
nullifier = SHA256("/LEZ/v1/multisig/vote/" || domain || 0x00 || nsk || proposal_index_le)
```

The `/LEZ/v1/multisig/vote/` prefix prevents collisions with other NSSA nullifier uses. The `domain` field (`"approve"`, `"reject"`, `"propose"`, `"execute"`) ensures that different actions on the same proposal produce different nullifiers.

**Properties:**
| Property | How it's achieved |
|----------|-------------------|
| No double-voting | Same NSK + same proposal + same domain = same nullifier (deduplicated on-chain) |
| Cross-action isolation | Different domain strings produce different nullifiers |
| Cross-proposal isolation | Different proposal_index values produce different nullifiers |
| Unlinkability | Different NSKs produce unrelated nullifiers (SHA256 preimage resistance) |

### Double-Vote Prevention

On-chain, each proposal stores `approved: Vec<[u8; 32]>` and `rejected: Vec<[u8; 32]>`. Before recording a vote, the handler checks that the nullifier is not already present:

```rust
let is_new = proposal.approve(nullifier);
assert!(is_new, "Member has already approved this proposal");
```

Since nullifiers are deterministic (same NSK + same inputs = same nullifier), a member cannot vote twice on the same proposal with the same action.

## Nonce Constraint Solution

### Problem

NSSA requires that any account modified in a transaction must have its current nonce included in the transaction's nonce list. Naive designs (e.g., storing a "last vote" marker in each member's account) would require the voter to include their account nonce — leaking which member account is voting and defeating privacy.

### Solution: NSK as ZK Witness Only

Member accounts are **never modified** by the multisig program. Voting works without touching member accounts:

1. **CreateMultisig** stores NPKs in the multisig state — members are registered by public key, not by account
2. **Propose/Approve/Reject/Execute** use unsigned transactions (no signer accounts in the witness set)
3. The only accounts modified are `multisig_state` and `proposal` PDAs, which are program-owned

This completely avoids the nonce problem: the voter's NSK is used only as a ZK witness inside the RISC0 prover, never as a transaction signer. The transaction includes nonces only for the multisig state and proposal PDAs.

```
Transaction accounts: [multisig_state_pda, proposal_pda]
Nonces:               [state_nonce]
Signers:              []  (empty — no private key signing)
Instruction:          Approve { proposal_index, vote_receipt, nullifier }
```

## Security Assumptions

1. **RISC0 soundness**: The ZK proof system (RISC0 zkVM) is assumed to be sound — a valid receipt can only be produced by running the guest code with inputs that satisfy the assertions. An attacker cannot forge a membership proof without knowing a valid NSK.

2. **NSK secrecy**: Each member's NSK must remain secret. If an NSK is compromised, the attacker can vote as that member. However, they cannot impersonate other members (each NSK produces a unique NPK).

3. **SHA256 preimage resistance**: The nullifier derivation relies on SHA256's preimage and collision resistance. An attacker cannot reverse-engineer an NSK from a nullifier, nor find two different inputs that produce the same nullifier.

4. **Member list is public**: The NPK list in MultisigState is public. An observer knows how many members exist and their NPKs, but not which members voted (only nullifiers are stored). The member list size leaks the total membership count.

5. **IMAGE_ID placeholder**: The current implementation uses `VOTE_CIRCUIT_IMAGE_ID = [0u32; 8]` — a placeholder. In production, this must be set to the actual RISC0 image ID of the vote_circuit guest binary to prevent forged proofs.

## Known Limitations

1. **IMAGE_ID hardcoded**: The vote circuit image ID is a placeholder `[0; 8]`. Upgrading requires deploying a new multisig program with the correct image ID. In a production system, this could be made upgradeable via a config-change proposal on the multisig itself.

2. **Member list public**: NPKs are stored in `MultisigState.members` on-chain. While NPKs don't reveal NSKs (one-way derivation), the membership set is visible. Future work could use Merkle commitments to hide the member list.

3. **No member rotation**: `AddMember`, `RemoveMember`, and `ChangeThreshold` instructions are not yet implemented. Changing membership requires deploying a new multisig.

4. **No proposal cleanup**: Executed and rejected proposals remain on-chain. A `CloseProposal` instruction would reclaim storage.

5. **Linear membership scan**: The circuit iterates over all members for the membership check. This is acceptable for the 10-member cap but would need optimization (e.g., Merkle proofs) for larger sets.

6. **Dev mode proofs**: With `RISC0_DEV_MODE=1`, proofs are not cryptographically sound. Production use requires real RISC0 proving.

7. **PDA derivation**: Uses legacy XOR-based seed derivation rather than the NSSA standard SHA256-prefix scheme.

## Integration Instructions

### Prerequisites

- Rust nightly (edition 2024)
- [RISC0 toolchain](https://dev.risczero.com/api/zkvm/install): `curl -L https://risczero.com/install | bash && rzup install`
- Docker (for reproducible guest builds)
- Running LEZ sequencer (localnet or testnet)
- Clone of [lssa](https://github.com/logos-blockchain/lssa) for wallet + token program

### Build

```bash
cd lez-multisig

# Build the guest binary (dev mode — fast, ~30s)
RISC0_DEV_MODE=1 cargo build -p multisig-methods --release

# Build the CLI
RISC0_DEV_MODE=1 cargo build -p multisig-cli --release

# The guest binary is at:
# target/riscv-guest/multisig-methods/multisig-guest/riscv32im-risc0-zkvm-elf/release/multisig.bin

# For production (reproducible, Docker-based — ~15-20 min):
# cargo risczero build --manifest-path methods/guest/Cargo.toml
```

### Deploy

```bash
# Start localnet
cd /tmp/spel-privacy-test && logos-scaffold localnet start

# Deploy multisig program
export NSSA_WALLET_HOME_DIR=~/lssa/wallet/configs/debug
wallet deploy-program target/riscv-guest/.../multisig.bin
```

### Run the Demo

```bash
# Full E2E demo (create → propose → approve → execute)
./scripts/demo-private-multisig.sh

# Unit tests
cargo test --lib

# E2E test (requires running sequencer)
MULTISIG_PROGRAM=<path-to-multisig.bin> \
TOKEN_PROGRAM=~/lssa/artifacts/program_methods/token.bin \
cargo test -p lez-multisig-e2e -- --nocapture
```

### CLI Usage

```bash
# Derive NPK from NSK
multisig derive-npk --nsk <64-hex-chars>

# Create multisig
multisig create --threshold 2 --members <npk1> <npk2> <npk3>

# Propose (any member)
multisig propose --multisig <create-key-base58> \
    --nsk <your-nsk-hex> \
    --members <npk1> <npk2> <npk3> \
    --target-program <program-id-hex> \
    --target-account-count 2 \
    --proposal-index 1

# Approve (any member)
multisig approve --multisig <create-key-base58> \
    --proposal 1 \
    --nsk <your-nsk-hex> \
    --members <npk1> <npk2> <npk3>

# Execute (any member, after threshold reached)
multisig execute --multisig <create-key-base58> \
    --proposal 1 \
    --nsk <your-nsk-hex> \
    --members <npk1> <npk2> <npk3>
```

## Benchmarks

### RISC0_DEV_MODE=1 (Development)

| Operation | Time |
|-----------|------|
| ZK proof generation (vote_circuit) | ~1-2s |
| Transaction submission + confirmation | ~15s (block time) |
| Full create→propose→approve→execute flow | ~60-90s |

Dev mode proofs are fast but not cryptographically sound. They are suitable for development and testing only.

### Production Proving (Estimated)

| Operation | Estimated Time |
|-----------|---------------|
| vote_circuit proof (CPU, single core) | ~30-60s |
| vote_circuit proof (GPU, CUDA) | ~5-10s |
| multisig guest proof (CPU) | ~2-5 min |
| multisig guest proof (GPU) | ~15-30s |

Production estimates are based on RISC0 v3.0.5 benchmarks for similar circuit complexity. Actual times depend on hardware and circuit size (member count affects the membership scan).

### Proof Size

| Component | Size |
|-----------|------|
| vote_circuit journal (nullifier + index) | 40 bytes |
| vote_receipt (serialized journal words) | ~10-20 u32 words |
| Full multisig instruction with proof | ~200-500 bytes |
