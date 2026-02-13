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

    // Args: <treasury.bin> <token.bin> <token_def_account_id>
    let treasury_path = std::env::args_os().nth(1)
        .expect("Usage: deploy_and_create_vault <treasury.bin> <token.bin> <token_def_id> <signer_id> [<signer_id_2> ...]")
        .into_string().unwrap();
    let token_path = std::env::args_os().nth(2)
        .expect("Missing <token.bin> path")
        .into_string().unwrap();
    let token_def_id: AccountId = std::env::args_os().nth(3)
        .expect("Missing <token_definition_account_id>")
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

    println!("Treasury program ID:    {:?}", treasury_program_id);
    println!("Token program ID:       {:?}", token_program_id);
    println!("Treasury state PDA:     {}", treasury_state_id);
    println!("Token definition:       {}", token_def_id);
    println!("Vault holding PDA:      {}", vault_holding_id);

    // All remaining args (from position 4 onward) are authorized signer account IDs.
    // Any of these accounts can authorize Send instructions.
    let authorized_signers: Vec<AccountId> = std::env::args_os()
        .skip(4)
        .map(|arg| arg.into_string().unwrap().parse::<AccountId>().expect("Invalid signer account ID"))
        .collect();

    if authorized_signers.is_empty() {
        eprintln!("Usage: deploy_and_create_vault <treasury.bin> <token.bin> <token_def_id> <signer_id> [<signer_id_2> ...]");
        eprintln!("At least one authorized signer is required.");
        std::process::exit(1);
    }

    println!("Authorized signers ({}):", authorized_signers.len());
    for (i, s) in authorized_signers.iter().enumerate() {
        println!("  [{}] {}", i, s);
    }

    // Build the CreateVault instruction
    let mut token_name = [0u8; 6];
    token_name[..4].copy_from_slice(b"TRSY");

    let authorized_accounts: Vec<[u8; 32]> = authorized_signers.iter()
        .map(|id| *id.value())
        .collect();

    let instruction = Instruction::CreateVault {
        token_name,
        initial_supply: 1_000_000,
        token_program_id,
        authorized_accounts,
    };

    // Message::try_new handles risc0 serialization internally
    let account_ids = vec![treasury_state_id, token_def_id, vault_holding_id];
    let nonces = vec![];
    let signing_keys = [];
    let message = Message::try_new(
        treasury_program_id,
        account_ids,
        nonces,
        instruction,
    ).unwrap();
    let witness_set = WitnessSet::for_message(&message, &signing_keys);
    let tx = PublicTransaction::new(message, witness_set);

    let response = wallet_core
        .sequencer_client
        .send_tx_public(tx)
        .await
        .unwrap();

    println!("\n📤 CreateVault transaction submitted!");
    println!("   Token 'TRSY' with supply 1000000 minted into vault.");
    println!("   tx_hash: {}", response.tx_hash);
    println!("   Waiting for confirmation...");

    let poller = wallet::poller::TxPoller::new(
        wallet_core.config().clone(),
        wallet_core.sequencer_client.clone(),
    );

    match poller.poll_tx(response.tx_hash).await {
        Ok(_) => println!("✅ Transaction confirmed — included in a block."),
        Err(e) => {
            eprintln!("❌ Transaction NOT confirmed: {e:#}");
            eprintln!("   It may have failed execution. Check sequencer logs for details.");
            std::process::exit(1);
        }
    }
}
