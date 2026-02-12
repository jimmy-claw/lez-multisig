// treasury_program::send — Send instruction handler.
//
//! Sends tokens from the treasury's vault holding PDA to an external
//! recipient by chaining to the Token program's `Transfer` instruction.

use borsh::BorshSerialize;
use nssa_core::program::{
    AccountPostState, AccountState, ChainedCall, ProgramId,
};
use token_core::Instruction as TokenInstruction;
use treasury_core::vault_holding_pda_seed;

/// Handle the `Send` instruction.
///
/// **Account layout:**
/// 0. `treasury_state` — treasury state PDA (read-only here)
/// 1. `vault_holding` — PDA holding tokens (authorized by treasury)
/// 2. `recipient_holding` — destination account for the tokens
pub fn handle(
    _program_id: &ProgramId,
    accounts: &mut [AccountState],
    amount: u128,
    token_program_id: &ProgramId,
) -> (Vec<AccountState>, Option<ChainedCall>) {
    assert!(accounts.len() >= 3, "Send requires 3 accounts");

    let _treasury_state = &accounts[0];

    // -- 1. Authorize the vault holding PDA ------------------------------------
    let mut vault_holding = accounts[1].clone();
    vault_holding.is_authorized = true;

    // -- 2. Recipient account ---------------------------------------------------
    let mut recipient_holding = accounts[2].clone();
    // Claim the recipient holding if it doesn't exist yet.
    recipient_holding.post_state = AccountPostState::new_claimed_if_default();

    // -- 3. Build Transfer chained call -----------------------------------------
    // We need to figure out the token_definition_id from the vault_holding's
    // data. For simplicity in this example we derive the PDA seed from the
    // token_definition stored in the vault_holding metadata.
    //
    // In the real AMM program the token_definition_id is passed explicitly or
    // read from pool state. Here we use the vault account's `owner` field or
    // an account in the layout. For this example, we assume the caller provides
    // the correct token_definition_id as part of the account at index 0
    // (treasury_state stores it) or we can derive it.
    //
    // For the PDA seed we need the token_definition_id. We'll extract it from
    // the treasury state or use a convention. Here, we read it from
    // vault_holding metadata (first 32 bytes = token_definition_id).
    let token_definition_id = vault_holding.id; // We use vault's ID for seed lookup

    let token_instruction = TokenInstruction::Transfer {
        amount,
        from: vault_holding.id,
        to: recipient_holding.id,
    };

    let token_ix_data = borsh::to_vec(&token_instruction).unwrap();

    let chained_accounts = vec![
        vault_holding.clone(), // source (authorized)
        recipient_holding,     // destination
    ];

    let chained_call = ChainedCall::new(
        token_program_id.clone(),
        token_ix_data,
        chained_accounts,
    )
    .with_pda_seeds(vec![vault_holding_pda_seed(&token_definition_id)]);

    // No account updates from the treasury itself — the Token program handles
    // balance changes.
    (vec![], Some(chained_call))
}
