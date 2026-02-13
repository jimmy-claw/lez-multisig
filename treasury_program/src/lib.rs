//! Treasury program — on-chain logic for PDA demonstration.
//!
//! This program demonstrates Program Derived Accounts (PDAs) without
//! depending on the Token program. It maintains an internal ledger.

pub mod create_vault;
pub mod deposit;
pub mod init;
pub mod transfer;
pub mod withdraw;

pub use treasury_core::Instruction;

use nssa_core::account::{Account, AccountId, AccountWithMetadata};
use nssa_core::program::{ProgramInput, ProgramOutput};
use treasury_core::{compute_treasury_state_pda, compute_vault_pda, TreasuryState, Vault};

/// Dispatch incoming instructions to their handlers.
pub fn process(
    program_id: &nssa_core::program::ProgramId,
    accounts: &mut [AccountWithMetadata],
    input_data: &[u8],
) -> ProgramOutput {
    // Deserialize the instruction
    let instruction = match Instruction::try_from_slice(input_data) {
        Ok(ix) => ix,
        Err(e) => {
            return ProgramOutput::error(format!("Failed to deserialize instruction: {}", e));
        }
    };

    match instruction {
        Instruction::Init => init::handle(accounts),
        Instruction::CreateVault { vault_name } => create_vault::handle(accounts, vault_name),
        Instruction::Deposit { amount } => deposit::handle(accounts, amount),
        Instruction::Withdraw { amount } => withdraw::handle(accounts, amount),
        Instruction::Transfer { amount } => transfer::handle(accounts, amount),
    }
}
