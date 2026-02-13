use nssa::{
    AccountId, PublicTransaction,
    program::Program,
    public_transaction::{Message, WitnessSet},
};
use treasury_core::{Instruction, compute_treasury_state_pda, compute_vault_holding_pda};
use wallet::WalletCore;

#[tokio::main]
async fn main() {
    let wallet_core = WalletCore::from_env().unwrap();

    // Args: <treasury.bin> <token.bin> <token_def_id> <recipient_id> <amount> <signer_id>
    let treasury_path = std::env::args_os().nth(1)
        .expect("Usage: send_from_vault <treasury.bin> <token.bin> <token_def_id> <recipient_id> <amount> <signer_id>")
        .into_string().unwrap();
    let token_path = std::env::args_os().nth(2)
        .expect("Missing <token.bin> path")
        .into_string().unwrap();
    let token_def_id: AccountId = std::env::args_os().nth(3)
        .expect("Missing <token_definition_account_id>")
        .into_string().unwrap()
        .parse().unwrap();
    let recipient_id: AccountId = std::env::args_os().nth(4)
        .expect("Missing <recipient_account_id>")
        .into_string().unwrap()
        .parse().unwrap();
    let amount: u128 = std::env::args_os().nth(5)
        .expect("Missing <amount>")
        .into_string().unwrap()
        .parse().unwrap();
    let signer_id: AccountId = std::env::args_os().nth(6)
        .expect("Missing <signer_account_id> — must be an authorized account from CreateVault")
        .into_string().unwrap()
        .parse().unwrap();

    // Load programs to get their IDs
    let treasury_bytecode: Vec<u8> = std::fs::read(&treasury_path).unwrap();
    let treasury_program = Program::new(treasury_bytecode).unwrap();
    let treasury_program_id = treasury_program.id();

    let token_bytecode: Vec<u8> = std::fs::read(&token_path).unwrap();
    let token_program = Program::new(token_bytecode).unwrap();
    let token_program_id = token_program.id();

    // Compute PDA account IDs
    let treasury_state_id = compute_treasury_state_pda(&treasury_program_id);
    let vault_holding_id = compute_vault_holding_pda(&treasury_program_id, &token_def_id);

    println!("Treasury state PDA:     {}", treasury_state_id);
    println!("Vault holding PDA:      {}", vault_holding_id);
    println!("Recipient:              {}", recipient_id);
    println!("Signer:                 {}", signer_id);
    println!("Amount:                 {}", amount);

    // Build the Send instruction
    let instruction = Instruction::Send {
        amount,
        token_program_id,
    };

    // Include signer_id as the 4th account — Send checks it's authorized
    let account_ids = vec![treasury_state_id, vault_holding_id, recipient_id, signer_id];

    // Fetch signer's current nonce from the sequencer
    let nonces = wallet_core.get_accounts_nonces(vec![signer_id]).await
        .expect("Failed to fetch nonce for signer account");

    // Get the signer's private key from the wallet storage
    let signing_key = wallet_core.storage.user_data
        .get_pub_account_signing_key(&signer_id)
        .expect("Signer private key not found in wallet — was this account created with `wallet account new public`?");

    let message = Message::try_new(
        treasury_program_id,
        account_ids,
        nonces,
        instruction,
    ).unwrap();
    let witness_set = WitnessSet::for_message(&message, &[signing_key]);
    let tx = PublicTransaction::new(message, witness_set);

    let _response = wallet_core
        .sequencer_client
        .send_tx_public(tx)
        .await
        .unwrap();

    println!("\n✅ Send transaction submitted!");
    println!("   {} tokens sent from vault.", amount);
}
