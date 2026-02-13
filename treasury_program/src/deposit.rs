//! Handler for the Deposit instruction.

use nssa_core::account::{AccountPostState, AccountWithMetadata};
use nssa_core::program::ProgramOutput;
use treasury_core::{TreasuryState, Vault};

/// Handle Deposit — add funds to a vault.
pub fn handle(accounts: &mut [AccountWithMetadata], amount: u64) -> ProgramOutput {
    if accounts.len() != 2 {
        return ProgramOutput::error(format!(
            "Deposit requires 2 accounts (treasury_state, vault), got {}",
            accounts.len()
        ));
    }

    let treasury_state = &mut accounts[0];
    let vault = &mut accounts[1];

    // Mark treasury state as accessed (no change)
    treasury_state.post_state = AccountPostState::new();

    // Update vault balance
    let mut vault_data = Vault::try_from_slice(&vault.account.data)
        .map_err(|e| format!("Failed to deserialize vault: {}", e))
        .unwrap();
    
    if !vault_data.initialized {
        return ProgramOutput::error("Vault not initialized".to_string());
    }
    
    vault_data.balance = vault_data.balance.saturating_add(amount);
    vault.account.data = borsh::to_vec(&vault_data).expect("Borsh serialize");
    vault.post_state = AccountPostState::new();

    ProgramOutput::success(vec![treasury_state.clone(), vault.clone()])
}
