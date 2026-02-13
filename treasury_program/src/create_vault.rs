//! Handler for the CreateVault instruction.

use nssa_core::account::{AccountPostState, AccountWithMetadata};
use nssa_core::program::ProgramOutput;
use treasury_core::{TreasuryState, Vault};

/// Handle CreateVault — create a new vault entry.
pub fn handle(accounts: &mut [AccountWithMetadata], vault_name: String) -> ProgramOutput {
    if accounts.len() != 2 {
        return ProgramOutput::error(format!(
            "CreateVault requires 2 accounts (treasury_state, vault), got {}",
            accounts.len()
        ));
    }

    let treasury_state = &mut accounts[0];
    let vault = &mut accounts[1];

    // Update treasury state vault count
    let mut state = TreasuryState::try_from_slice(&treasury_state.account.data)
        .map_err(|e| format!("Failed to deserialize treasury state: {}", e))
        .unwrap();
    state.vault_count += 1;
    treasury_state.account.data = borsh::to_vec(&state).expect("Borsh serialize");
    treasury_state.post_state = AccountPostState::new();

    // Initialize the vault
    let vault_data = Vault {
        name: vault_name,
        balance: 0,
        initialized: true,
    };
    vault.account.data = borsh::to_vec(&vault_data).expect("Borsh serialize");
    vault.post_state = AccountPostState::new_claimed();

    ProgramOutput::success(vec![treasury_state.clone(), vault.clone()])
}
