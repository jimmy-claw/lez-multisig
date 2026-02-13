// treasury_core — shared types and PDA derivation helpers for the Treasury program.
//
//! This crate contains the instruction enum, vault state, and PDA seed helpers
//! used by both the on-chain guest program and off-chain tooling.

use borsh::{BorshDeserialize, BorshSerialize};
use nssa_core::account::AccountId;
use nssa_core::program::{PdaSeed, ProgramId};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Instructions
// ---------------------------------------------------------------------------

/// Instructions that the Treasury program understands.
///
/// This is a simplified treasury that demonstrates PDA patterns without
/// depending on the Token program. It maintains an internal ledger of balances.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Instruction {
    /// Initialize the treasury state (first-time setup).
    ///
    /// **Accounts:**
    /// 0. `treasury_state` — PDA that will hold treasury metadata
    Init,

    /// Create a new vault entry for tracking a token balance.
    ///
    /// **Accounts:**
    /// 0. `treasury_state` — treasury state PDA
    /// 1. `vault` — PDA that will hold the vault data
    CreateVault {
        /// Name/identifier for this vault
        vault_name: String,
    },

    /// Deposit funds into a vault.
    ///
    /// **Accounts:**
    /// 0. `treasury_state` — treasury state PDA
    /// 1. `vault` — vault PDA to credit
    Deposit {
        /// Amount to deposit
        amount: u64,
    },

    /// Withdraw funds from a vault.
    ///
    /// **Accounts:**
    /// 0. `treasury_state` — treasury state PDA
    /// 1. `vault` — vault PDA to debit
    Withdraw {
        /// Amount to withdraw
        amount: u64,
    },

    /// Transfer between vaults.
    ///
    /// **Accounts:**
    /// 0. `treasury_state` — treasury state PDA
    /// 1. `from_vault` — source vault PDA
    /// 2. `to_vault` — destination vault PDA
    Transfer {
        /// Amount to transfer
        amount: u64,
    },
}

// ---------------------------------------------------------------------------
// Vault state
// ---------------------------------------------------------------------------

/// State stored in the treasury_state PDA.
#[derive(Debug, Clone, Default, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct TreasuryState {
    /// How many vaults have been created.
    pub vault_count: u64,
}

/// State stored in each vault PDA.
#[derive(Debug, Clone, Default, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct Vault {
    /// Name of this vault.
    pub name: String,
    /// Current balance.
    pub balance: u64,
    /// Whether this vault has been initialized.
    pub initialized: bool,
}

// ---------------------------------------------------------------------------
// PDA derivation helpers
// ---------------------------------------------------------------------------

/// Fixed 32-byte seed for treasury state PDA.
const TREASURY_STATE_SEED: [u8; 32] = {
    let mut seed = [0u8; 32];
    let tag = b"treasury_state";
    let mut i = 0;
    while i < tag.len() {
        seed[i] = tag[i];
        i += 1;
    }
    seed
};

/// Fixed 32-byte seed prefix for vault PDAs.
const VAULT_SEED_PREFIX: [u8; 32] = {
    let mut seed = [0u8; 32];
    let tag = b"vault";
    let mut i = 0;
    while i < tag.len() {
        seed[i] = tag[i];
        i += 1;
    }
    seed
};

/// Compute the treasury state PDA account ID.
pub fn compute_treasury_state_pda(treasury_program_id: &ProgramId) -> AccountId {
    AccountId::from((treasury_program_id, &treasury_state_pda_seed()))
}

/// Compute a vault PDA account ID by name.
pub fn compute_vault_pda(treasury_program_id: &ProgramId, vault_name: &str) -> AccountId {
    // Combine prefix + name, pad to 32 bytes
    let mut seed = VAULT_SEED_PREFIX;
    for (i, byte) in vault_name.as_bytes().iter().take(16).enumerate() {
        seed[i] = *byte;
    }
    AccountId::from((treasury_program_id, &PdaSeed::new(seed)))
}

/// Build the [`PdaSeed`] for treasury state.
pub fn treasury_state_pda_seed() -> PdaSeed {
    PdaSeed::new(TREASURY_STATE_SEED)
}

/// Build the [`PdaSeed`] for a vault.
pub fn vault_pda_seed(vault_name: &str) -> PdaSeed {
    let mut seed = VAULT_SEED_PREFIX;
    for (i, byte) in vault_name.as_bytes().iter().take(16).enumerate() {
        seed[i] = *byte;
    }
    PdaSeed::new(seed)
}
