//! Deploy treasury and create vault

use nssa::{
    AccountId, PublicTransaction,
    program::Program,
    public_transaction::{Message, WitnessSet},
};
use treasury_core::compute_treasury_pda;
use wallet::WalletCore;

#[tokio::main]
async fn main() {
    let wallet_core = WalletCore::from_env().unwrap();

    // Args: <treasury.bin> <token.bin> <token_def_account_id>
    let treasury_path = std::env::args_os().nth(1).unwrap().into_string().unwrap();
    let _token_path = std::env::args_os().nth(2).unwrap().into_string().unwrap();
    let token_def_id: AccountId = std::env::args_os().nth(3).unwrap().into_string().unwrap().parse().unwrap();

    let treasury_bytecode = std::fs::read(&treasury_path).unwrap();
    let treasury_program = Program::new(treasury_bytecode).unwrap();
    let treasury_program_id = treasury_program.id();

    let treasury_pda = compute_treasury_pda(&treasury_program_id);

    println!("Treasury program ID: {:?}", treasury_program_id);
    println!("Treasury PDA: {}", treasury_pda);
    println!("Token definition: {}", token_def_id);

    // Send empty Vec<u8> as instruction - matches guest type
    let instruction: Vec<u8> = vec![];

    let account_ids = vec![treasury_pda, token_def_id];
    let nonces = vec![];
    let signing_keys = [];
    let message = Message::try_new(treasury_program_id, account_ids, nonces, instruction).unwrap();
    let witness_set = WitnessSet::for_message(&message, &signing_keys);
    let tx = PublicTransaction::new(message, witness_set);

    let _response = wallet_core
        .sequencer_client
        .send_tx_public(tx)
        .await
        .unwrap();

    println!("✅ Treasury executed successfully!");
}
