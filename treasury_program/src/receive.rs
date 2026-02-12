// treasury_program::receive — Deposit instruction handler.
//
//! Receives (deposits) tokens from an external sender into the treasury's
//! vault holding PDA by chaining to the Token program's `Transfer` instruction.

use borsh::BorshSerialize;
use nssa_core::program::{
    AccountPostState, AccountState, ChainedCall, ProgramId,
};
use token_core::Instruction as TokenInstruction;
use treasury_core::vault_holding_pda_seed;

/// Handle the `Deposit` instruction.
///
/// **Account layout:**
/// 0. `treasury_state` — treasury state PDA (read-only here)
/// 1. `sender_holding` — sender's token holding (authorized by the user/caller)
/// 2. `vault_holding` — PDA that will receive the tokens
pub fn handle(
    _program_id: &ProgramId,
    accounts: &mut [AccountState],
    amount: u128,
    token_program_id: &ProgramId,
) -> (Vec<AccountState>, Option<ChainedCall>) {
    assert!(accounts.len() >= 3, "Deposit requires 3 accounts");

    let _treasury_state = &accounts[0];

    // -- 1. Sender holding (already authorized by the caller) -------------------
    let sender_holding = accounts[1].clone();

    // -- 2. Vault holding PDA ---------------------------------------------------
    let mut vault_holding = accounts[2].clone();
    // Claim the vault if this is the first deposit.
    vault_holding.post_state = AccountPostState::new_claimed_if_default();

    // The vault does NOT need `is_authorized = true` here because it is the
    // *receiver* — the Token program only requires authorization on the source.

    // -- 3. Build Transfer chained call -----------------------------------------
    let token_instruction = TokenInstruction::Transfer {
        amount,
        from: sender_holding.id,
        to: vault_holding.id,
    };

    let token_ix_data = borsh::to_vec(&token_instruction).unwrap();

    let chained_accounts = vec![
        sender_holding, // source (authorized by original caller)
        vault_holding,  // destination (PDA, no auth needed for receives)
    ];

    let chained_call = ChainedCall::new(
        token_program_id.clone(),
        token_ix_data,
        chained_accounts,
    );
    // Note: no .with_pda_seeds() needed here because the vault is the
    // *receiver*. The sender's authorization comes from the original caller,
    // not from the treasury program.

    (vec![], Some(chained_call))
}
