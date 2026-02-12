// Example: Send tokens from a treasury vault to a recipient.
//
// This example shows how to:
// 1. Derive the vault holding PDA for a given token
// 2. Build a Send instruction
// 3. Construct the ProgramInput with the correct account layout
//
// NOTE: This is a conceptual example. Actual execution requires a running
// NSSA node.

use borsh::BorshSerialize;
use nssa_core::program::{AccountId, AccountState, ProgramId, ProgramInput};
use treasury_core::{
    Instruction, compute_treasury_state_pda, compute_vault_holding_pda,
};

fn main() {
    println!("=== Treasury: Send From Vault ===\n");

    // --- Program IDs (would come from deployment) ---
    let treasury_program_id = ProgramId::from_bytes(&[1u8; 32]);
    let token_program_id = ProgramId::from_bytes(&[2u8; 32]);

    // --- Derive accounts ---
    let treasury_state_id = compute_treasury_state_pda(&treasury_program_id);
    let token_definition_id = AccountId::from_bytes(&[3u8; 32]);
    let vault_holding_id =
        compute_vault_holding_pda(&treasury_program_id, &token_definition_id);

    // Recipient is an arbitrary account (e.g., a user's token holding).
    let recipient_id = AccountId::from_bytes(&[4u8; 32]);

    println!("Vault Holding PDA: {:?}", vault_holding_id);
    println!("Recipient:         {:?}", recipient_id);

    // --- Build Send instruction ---
    let instruction = Instruction::Send {
        amount: 500,
        token_program_id: token_program_id.clone(),
    };

    let ix_data = borsh::to_vec(&instruction).expect("serialize instruction");

    // --- Build accounts ---
    // The treasury program expects:
    //   [0] treasury_state (PDA)
    //   [1] vault_holding (PDA, will be authorized by treasury)
    //   [2] recipient_holding
    let accounts = vec![
        AccountState::new(treasury_state_id),
        AccountState::new(vault_holding_id),
        AccountState::new(recipient_id),
    ];

    let program_input = ProgramInput {
        program_id: treasury_program_id,
        accounts,
        input_data: ix_data,
    };

    println!("\nProgramInput built for Send instruction.");
    println!("Amount: 500 tokens");
    println!("Accounts: {}", program_input.accounts.len());

    println!("\n--- What happens inside the zkVM ---");
    println!("1. Guest binary reads ProgramInput");
    println!("2. treasury_program::process() dispatches to send::handle()");
    println!("3. vault_holding.is_authorized = true (treasury authorizes its PDA)");
    println!("4. A ChainedCall to Token::Transfer is returned");
    println!("5. The runtime executes the transfer: vault → recipient");
    println!("6. PDA seed proves the treasury owns the vault account");
}
