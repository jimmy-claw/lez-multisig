//! Handler for the Init instruction.

use nssa_core::account::{AccountPostState, AccountWithMetadata};
use nssa_core::program::ProgramOutput;
use treasury_core::{compute_treasury_state_pda, TreasuryState};

/// Handle the Init instruction — initialize treasury state PDA.
pub fn handle(accounts: &mut [AccountWithMetadata]) -> ProgramOutput {
    if accounts.len() != 1 {
        return ProgramOutput::error(format!(
            "Init requires 1 account (treasury_state), got {}",
            accounts.len()
        ));
    }

    let treasury_state = &mut accounts[0];

    // Initialize the treasury state
    let state = TreasuryState { vault_count: 0 };
    let state_bytes = borsh::to_vec(&state).expect("Borsh serialize");

    treasury_state.account.data = state_bytes;
    treasury_state.post_state = AccountPostState::new_claimed();

    ProgramOutput::success(vec![treasury_state.clone()])
}
