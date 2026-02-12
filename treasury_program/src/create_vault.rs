// treasury_program::create_vault — CreateVault instruction handler.
//
//! Creates the treasury state PDA (if first time), then chains to the Token
//! program's `NewFungibleDefinition` instruction to create a new token and
//! mint the initial supply into the treasury's vault holding PDA.

use borsh::BorshSerialize;
use nssa_core::program::{
    AccountPostState, AccountState, ChainedCall, ProgramId,
};
use token_core::Instruction as TokenInstruction;
use treasury_core::{
    TreasuryState, treasury_state_pda_seed, vault_holding_pda_seed,
};

/// Handle the `CreateVault` instruction.
///
/// **Account layout:**
/// 0. `treasury_state` — treasury state PDA (claimed on first call)
/// 1. `token_definition` — will become the new token definition PDA
/// 2. `vault_holding` — will receive the initial minted supply
pub fn handle(
    _program_id: &ProgramId,
    accounts: &mut [AccountState],
    token_name: String,
    initial_supply: u128,
    treasury_program_id: &ProgramId,
    token_program_id: &ProgramId,
) -> (Vec<AccountState>, Option<ChainedCall>) {
    assert!(accounts.len() >= 3, "CreateVault requires 3 accounts");

    // -- 1. Treasury state PDA --------------------------------------------------
    let treasury_state_account = &mut accounts[0];

    // Deserialize existing state or start fresh.
    let mut state: TreasuryState = if treasury_state_account.data.is_empty() {
        TreasuryState::default()
    } else {
        borsh::from_slice(&treasury_state_account.data)
            .expect("failed to deserialize TreasuryState")
    };

    state.vault_count += 1;

    // Update the account data.
    treasury_state_account.data = borsh::to_vec(&state).unwrap();
    // Claim on first use; no-op if already claimed.
    treasury_state_account.post_state = AccountPostState::new_claimed_if_default();

    // -- 2. Token definition PDA ------------------------------------------------
    let token_definition = &mut accounts[1];
    token_definition.post_state = AccountPostState::new_claimed();

    // -- 3. Vault holding PDA ---------------------------------------------------
    let mut vault_holding = accounts[2].clone();
    vault_holding.is_authorized = true; // authorize so Token program can write to it
    vault_holding.post_state = AccountPostState::new_claimed();

    // -- 4. Build the chained call to the Token program -------------------------
    //
    // We invoke Token::NewFungibleDefinition which creates the token and mints
    // the initial supply into `vault_holding`.
    let token_instruction = TokenInstruction::NewFungibleDefinition {
        name: token_name,
        initial_supply,
        receiver: vault_holding.id,
    };

    let token_ix_data = borsh::to_vec(&token_instruction).unwrap();

    // The chained call needs the token_definition and vault_holding accounts.
    let chained_accounts = vec![
        accounts[1].clone(), // token_definition
        vault_holding,       // vault_holding (authorized)
    ];

    let chained_call = ChainedCall::new(
        token_program_id.clone(),
        token_ix_data,
        chained_accounts,
    )
    // Authorize the vault holding PDA so the Token program accepts writes.
    .with_pda_seeds(vec![vault_holding_pda_seed(&accounts[1].id)]);

    // Return updated accounts and the chained call.
    let updated_accounts = vec![accounts[0].clone()];
    (updated_accounts, Some(chained_call))
}
