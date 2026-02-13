//! Handler for the Transfer instruction.

use nssa_core::account::{AccountPostState, AccountWithMetadata};
use nssa_core::program::ProgramOutput;
use treasury_core::{TreasuryState, Vault};

/// Handle Transfer — move funds between two vaults.
pub fn handle(accounts: &mut [AccountWithMetadata], amount: u64) -> ProgramOutput {
    if accounts.len() != 3 {
        return ProgramOutput::error(format!(
            "Transfer requires 3 accounts (treasury_state, from_vault, to_vault), got {}",
            accounts.len()
        ));
    }

    let treasury_state = &mut accounts[0];
    let from_vault = &mut accounts[1];
    let to_vault = &mut accounts[2];

    treasury_state.post_state = AccountPostState::new();

    // Debit from source vault
    let mut from_data = Vault::try_from_slice(&from_vault.account.data)
        .map_err(|e| format!("Failed to deserialize from_vault: {}", e))
        .unwrap();
    
    if !from_data.initialized {
        return ProgramOutput::error("Source vault not initialized".to_string());
    }
    
    if from_data.balance < amount {
        return ProgramOutput::error(format!(
            "Insufficient balance: have {}, need {}",
            from_data.balance, amount
        ));
    }
    
    from_data.balance = from_data.balance.saturating_sub(amount);
    from_vault.account.data = borsh::to_vec(&from_data).expect("Borsh serialize");
    from_vault.post_state = AccountPostState::new();

    // Credit to destination vault
    let mut to_data = Vault::try_from_slice(&to_vault.account.data)
        .map_err(|e| format!("Failed to deserialize to_vault: {}", e))
        .unwrap();
    
    if !to_data.initialized {
        return ProgramOutput::error("Destination vault not initialized".to_string());
    }
    
    to_data.balance = to_data.balance.saturating_add(amount);
    to_vault.account.data = borsh::to_vec(&to_data).expect("Borsh serialize");
    to_vault.post_state = AccountPostState::new();

    ProgramOutput::success(vec![
        treasury_state.clone(),
        from_vault.clone(),
        to_vault.clone(),
    ])
}
