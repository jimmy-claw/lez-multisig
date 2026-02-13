//! Example: Initialize treasury and create vaults.
//!
//! Usage:
//!   cargo run --bin init_treasury <path/to/treasury.bin>
//!   cargo run --bin create_vault <path/to/treasury.bin> <vault_name>
//!   cargo run --bin deposit <path/to/treasury.bin> <vault_name> <amount>
//!   cargo run --bin withdraw <path/to/treasury.bin> <vault_name> <amount>
//!   cargo run --bin transfer <path/to/treasury.bin> <from_vault> <to_vault> <amount>

use nssa::{
    AccountId, PublicTransaction,
    program::Program,
    public_transaction::{Message, WitnessSet},
};
use treasury_core::{compute_treasury_state_pda, compute_vault_pda, Instruction};
use wallet::WalletCore;

#[tokio::main]
async fn main() {
    let wallet_core = WalletCore::from_env().unwrap();

    // Get the binary path from args
    let bin_path = std::env::args_os()
        .nth(1)
        .expect("Usage: <treasury.bin> [args...]")
        .into_string()
        .unwrap();

    // Load the treasury program
    let bytecode = std::fs::read(&bin_path).unwrap();
    let program = Program::new(bytecode).unwrap();
    let program_id = program.id();

    // Compute PDA
    let treasury_state_pda = compute_treasury_state_pda(&program_id);

    println!("Treasury program ID: {:?}", program_id);
    println!("Treasury state PDA:  {}", treasury_state_pda);

    // Parse subcommand
    let subcommand = std::env::args_os()
        .nth(2)
        .unwrap_or_default()
        .into_string()
        .unwrap_or_default();

    match subcommand.as_str() {
        "init" => run_init(&wallet_core, program_id, treasury_state_pda).await,
        "create_vault" => {
            let vault_name = std::env::args_os()
                .nth(3)
                .expect("Usage: create_vault <vault_name>")
                .into_string()
                .unwrap();
            run_create_vault(&wallet_core, program_id, treasury_state_pda, &vault_name).await
        }
        "deposit" => {
            let vault_name = std::env::args_os()
                .nth(3)
                .expect("Usage: deposit <vault_name> <amount>")
                .into_string()
                .unwrap();
            let amount: u64 = std::env::args_os()
                .nth(4)
                .expect("Missing amount")
                .into_string()
                .unwrap()
                .parse()
                .unwrap();
            run_deposit(&wallet_core, program_id, treasury_state_pda, &vault_name, amount).await
        }
        "withdraw" => {
            let vault_name = std::env::args_os()
                .nth(3)
                .expect("Usage: withdraw <vault_name> <amount>")
                .into_string()
                .unwrap();
            let amount: u64 = std::env::args_os()
                .nth(4)
                .expect("Missing amount")
                .into_string()
                .unwrap()
                .parse()
                .unwrap();
            run_withdraw(&wallet_core, program_id, treasury_state_pda, &vault_name, amount).await
        }
        "transfer" => {
            let from_vault = std::env::args_os()
                .nth(3)
                .expect("Usage: transfer <from_vault> <to_vault> <amount>")
                .into_string()
                .unwrap();
            let to_vault = std::env::args_os()
                .nth(4)
                .expect("Missing <to_vault>")
                .into_string()
                .unwrap();
            let amount: u64 = std::env::args_os()
                .nth(5)
                .expect("Missing amount")
                .into_string()
                .unwrap()
                .parse()
                .unwrap();
            run_transfer(&wallet_core, program_id, treasury_state_pda, &from_vault, &to_vault, amount).await
        }
        _ => {
            println!("Subcommands: init, create_vault, deposit, withdraw, transfer");
            println!("Example: cargo run --bin treasury_examples treasury.bin init");
        }
    }
}

async fn run_init(wallet: &WalletCore, program_id: Program, treasury_pda: AccountId) {
    let instruction = Instruction::Init;
    send_tx(wallet, program_id, vec![treasury_pda], instruction).await;
    println!("✅ Treasury initialized!");
}

async fn run_create_vault(wallet: &WalletCore, program_id: Program, treasury_pda: AccountId, name: &str) {
    let vault_pda = compute_vault_pda(&program_id, name);
    println!("Creating vault '{}' at: {}", name, vault_pda);

    let instruction = Instruction::CreateVault {
        vault_name: name.to_string(),
    };
    send_tx(wallet, program_id, vec![treasury_pda, vault_pda], instruction).await;
    println!("✅ Vault '{}' created!", name);
}

async fn run_deposit(wallet: &WalletCore, program_id: Program, treasury_pda: AccountId, name: &str, amount: u64) {
    let vault_pda = compute_vault_pda(&program_id, name);
    println!("Depositing {} to vault '{}' at {}", amount, name, vault_pda);

    let instruction = Instruction::Deposit { amount };
    send_tx(wallet, program_id, vec![treasury_pda, vault_pda], instruction).await;
    println!("✅ Deposited {} to vault '{}'!", amount, name);
}

async fn run_withdraw(wallet: &WalletCore, program_id: Program, treasury_pda: AccountId, name: &str, amount: u64) {
    let vault_pda = compute_vault_pda(&program_id, name);
    println!("Withdrawing {} from vault '{}'", amount, name);

    let instruction = Instruction::Withdraw { amount };
    send_tx(wallet, program_id, vec![treasury_pda, vault_pda], instruction).await;
    println!("✅ Withdrew {} from vault '{}'!", amount, name);
}

async fn run_transfer(wallet: &WalletCore, program_id: Program, treasury_pda: AccountId, from: &str, to: &str, amount: u64) {
    let from_pda = compute_vault_pda(&program_id, from);
    let to_pda = compute_vault_pda(&program_id, to);
    println!("Transferring {} from '{}' to '{}'", amount, from, to);

    let instruction = Instruction::Transfer { amount };
    send_tx(wallet, program_id, vec![treasury_pda, from_pda, to_pda], instruction).await;
    println!("✅ Transferred {} from '{}' to '{}'!", amount, from, to);
}

async fn send_tx(wallet: &WalletCore, program_id: Program, account_ids: Vec<AccountId>, instruction: Instruction) {
    let instruction_data = risc0_zkvm::serde::to_vec(&instruction).unwrap();
    let instruction_bytes: Vec<u8> = instruction_data
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();

    let message = Message::try_new(program_id, account_ids, vec![], instruction_bytes).unwrap();
    let witness_set = WitnessSet::for_message(&message, &[]);
    let tx = PublicTransaction::new(message, witness_set);

    let _response = wallet
        .sequencer_client
        .send_tx_public(tx)
        .await
        .unwrap();
}
