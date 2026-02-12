// Example: Deploy the Treasury program and create a vault.
//
// This example shows how to:
// 1. Build the program image from the guest ELF
// 2. Deploy it to the NSSA runtime
// 3. Call CreateVault to create a new token + vault
//
// NOTE: This is a conceptual example. Actual deployment requires a running
// NSSA node and the correct RPC setup.

use borsh::BorshSerialize;
use nssa_core::program::{AccountId, AccountState, ProgramId, ProgramInput};
use treasury_core::{
    Instruction, compute_treasury_state_pda, compute_vault_holding_pda,
};

fn main() {
    println!("=== Treasury: Deploy and Create Vault ===\n");

    // --- Step 1: Define program IDs ---
    // In a real deployment, the treasury_program_id comes from deploying the
    // guest ELF binary. Here we use a placeholder.
    let treasury_program_id = ProgramId::from_bytes(&[1u8; 32]);
    let token_program_id = ProgramId::from_bytes(&[2u8; 32]);

    println!("Treasury Program ID: {:?}", treasury_program_id);
    println!("Token Program ID:    {:?}", token_program_id);

    // --- Step 2: Derive PDA account IDs ---
    let treasury_state_id = compute_treasury_state_pda(&treasury_program_id);
    println!("\nTreasury State PDA:  {:?}", treasury_state_id);

    // For the token definition, we'd normally get this from the Token program.
    // Here we use a placeholder to show the derivation.
    let token_definition_id = AccountId::from_bytes(&[3u8; 32]);
    let vault_holding_id =
        compute_vault_holding_pda(&treasury_program_id, &token_definition_id);
    println!("Vault Holding PDA:   {:?}", vault_holding_id);

    // --- Step 3: Build the CreateVault instruction ---
    let instruction = Instruction::CreateVault {
        token_name: "TreasuryToken".to_string(),
        initial_supply: 1_000_000,
        treasury_program_id: treasury_program_id.clone(),
        token_program_id: token_program_id.clone(),
    };

    let ix_data = borsh::to_vec(&instruction).expect("serialize instruction");
    println!("\nInstruction data ({} bytes): {:?}", ix_data.len(), &ix_data[..20]);

    // --- Step 4: Build the accounts list ---
    let accounts = vec![
        AccountState::new(treasury_state_id),   // treasury state PDA
        AccountState::new(token_definition_id), // token definition PDA
        AccountState::new(vault_holding_id),    // vault holding PDA
    ];

    // --- Step 5: Build ProgramInput ---
    let program_input = ProgramInput {
        program_id: treasury_program_id,
        accounts,
        input_data: ix_data,
    };

    println!("\nProgramInput built successfully.");
    println!("Accounts: {}", program_input.accounts.len());
    println!(
        "In a real deployment, this would be submitted to the NSSA runtime \
         which would execute the guest binary inside the zkVM."
    );

    // --- Step 6: What happens next ---
    println!("\n--- What happens inside the zkVM ---");
    println!("1. Guest binary reads ProgramInput");
    println!("2. treasury_program::process() dispatches to create_vault::handle()");
    println!("3. TreasuryState is initialized (vault_count = 1)");
    println!("4. A ChainedCall to Token::NewFungibleDefinition is returned");
    println!("5. The runtime executes the chained call in the Token program");
    println!("6. Token program creates the definition and mints to vault PDA");
}
