//! Handler for the Withdraw instruction.

use nssa_core::account::{AccountPostState, AccountWithMetadata};
use nssa_core::program::ProgramOutput;
use treasury_core::{TreasuryState, Vault};

/// Handle Withdraw — remove funds from a vault.
pub fn handle(accounts: &mut [AccountWithMetadata], amount: u64) -> ProgramOutput {
    if accounts.len() != 2 {
        return ProgramOutput::error(format!(
            "Withdraw requires 2 accounts (treasury_state, vault), got {}",
            accounts.len()
        ));
    }

    let treasury_state = &mut accounts[0];
    let vault = &mut accounts[1];

    treasury_state.post_state = AccountPostState::new();

    // Update vault balance
    let mut vault_data = Vault::try_from_slice(&vault.account.data)
        .map_err(|e| format!("Failed to deserialize vault: {}", e))
        .unwrap();
    
    if !vault_data.initialized {
        return ProgramOutput::error("Vault not initialized".to_string());
    }
    
    if vault_data.balance < amount {
        return ProgramOutput::error(format!(
            "Insufficient balance: have {}, need {}",
            vault_data.balance, amount
        ));
    }
    
    vault_data.balance = vault_data.balance.saturating_sub(amount);
    vault.account.data = borsh::to_vec(&vault_data).expect("Borsh serialize");
    vault.post_state = AccountPostState::new();

    ProgramOutput::success(vec![treasury_state.clone(), vault.clone()])
}
