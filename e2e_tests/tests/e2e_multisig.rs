//! End-to-end tests for the multisig program (Squads-style on-chain proposals).
//!
//! Prerequisites:
//! - A running sequencer at SEQUENCER_URL (default http://127.0.0.1:3040)
//! - MULTISIG_PROGRAM env var pointing to the compiled guest binary
//!
//! Run with: `cargo test -p lez-multisig-e2e --test e2e_multisig -- --nocapture --test-threads=1`

use std::time::Duration;

use nssa::{
    AccountId, PrivateKey, ProgramDeploymentTransaction, PublicKey, PublicTransaction,
    program::Program,
    public_transaction::{Message, WitnessSet},
};
use multisig_core::{Instruction, MultisigState, ProposalAction, ProposalStatus, compute_multisig_state_pda};
use common::sequencer_client::SequencerClient;

const BLOCK_WAIT_SECS: u64 = 15;

fn account_id_from_key(key: &PrivateKey) -> AccountId {
    let pk = PublicKey::new_from_private_key(key);
    AccountId::from(&pk)
}

fn load_program_bytecode() -> Vec<u8> {
    let path = std::env::var("MULTISIG_PROGRAM")
        .unwrap_or_else(|_| "target/riscv32im-risc0-zkvm-elf/docker/multisig.bin".to_string());
    std::fs::read(&path)
        .unwrap_or_else(|_| panic!("Cannot read program binary at '{}'", path))
}

fn sequencer_client() -> SequencerClient {
    let url = std::env::var("SEQUENCER_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:3040".to_string());
    SequencerClient::new(url.parse().unwrap()).expect("Failed to create sequencer client")
}

async fn submit_tx(client: &SequencerClient, tx: PublicTransaction) {
    let response = client.send_tx_public(tx).await.expect("Failed to submit tx");
    println!("  tx_hash: {}", response.tx_hash);
    tokio::time::sleep(Duration::from_secs(BLOCK_WAIT_SECS)).await;
}

/// Submit a single-signer transaction
async fn submit_signed(
    client: &SequencerClient,
    program_id: nssa::ProgramId,
    account_ids: Vec<AccountId>,
    signer_key: &PrivateKey,
    nonces: Vec<u128>,
    instruction: Instruction,
) {
    let message = Message::try_new(program_id, account_ids, nonces, instruction).unwrap();
    let witness_set = WitnessSet::for_message(&message, &[signer_key]);
    let tx = PublicTransaction::new(message, witness_set);
    submit_tx(client, tx).await;
}

async fn get_nonce(client: &SequencerClient, account_id: AccountId) -> u128 {
    client.get_account(account_id).await
        .map(|r| r.account.nonce)
        .unwrap_or(0)
}

async fn get_multisig_state(client: &SequencerClient, state_id: AccountId) -> MultisigState {
    let account = client.get_account(state_id).await.expect("Failed to get multisig state");
    borsh::from_slice(&account.account.data).expect("Failed to deserialize multisig state")
}

/// Deploy program and return its ID (skips if already deployed)
async fn deploy_and_get_id(client: &SequencerClient) -> nssa::ProgramId {
    let bytecode = load_program_bytecode();
    let program = Program::new(bytecode.clone()).expect("Invalid program");
    let program_id = program.id();
    let pda = compute_multisig_state_pda(&program_id);

    // Check if already deployed by querying the PDA
    // If the program is deployed, the PDA account should be fetchable (even if empty)
    println!("📦 Deploying multisig program (or reusing existing)...");
    let deploy_msg = nssa::program_deployment_transaction::Message::new(bytecode);
    let deploy_tx = ProgramDeploymentTransaction::new(deploy_msg);
    match client.send_tx_program(deploy_tx).await {
        Ok(response) => {
            println!("  deploy tx_hash: {}", response.tx_hash);
            tokio::time::sleep(Duration::from_secs(BLOCK_WAIT_SECS)).await;
        }
        Err(e) => {
            println!("  deploy skipped (likely already exists): {}", e);
        }
    }

    program_id
}

#[tokio::test]
async fn test_create_and_query_multisig() {
    let client = sequencer_client();
    let program_id = deploy_and_get_id(&client).await;
    let multisig_state_id = compute_multisig_state_pda(&program_id);

    let key1 = PrivateKey::new_os_random();
    let key2 = PrivateKey::new_os_random();
    let key3 = PrivateKey::new_os_random();
    let m1 = account_id_from_key(&key1);
    let m2 = account_id_from_key(&key2);
    let m3 = account_id_from_key(&key3);

    println!("🔐 Creating 2-of-3 multisig...");
    let instruction = Instruction::CreateMultisig {
        threshold: 2,
        members: vec![*m1.value(), *m2.value(), *m3.value()],
    };
    let message = Message::try_new(program_id, vec![multisig_state_id], vec![], instruction).unwrap();
    let witness_set = WitnessSet::for_message(&message, &[] as &[&PrivateKey]);
    submit_tx(&client, PublicTransaction::new(message, witness_set)).await;

    // Verify on-chain
    let state = get_multisig_state(&client, multisig_state_id).await;
    assert_eq!(state.threshold, 2);
    assert_eq!(state.members.len(), 3);
    assert_eq!(state.transaction_index, 0);
    assert!(state.proposals.is_empty());
    println!("✅ Multisig created!");
}

#[tokio::test]
async fn test_propose_approve_execute_transfer() {
    let client = sequencer_client();
    let program_id = deploy_and_get_id(&client).await;
    let multisig_state_id = compute_multisig_state_pda(&program_id);

    let key1 = PrivateKey::new_os_random();
    let key2 = PrivateKey::new_os_random();
    let key3 = PrivateKey::new_os_random();
    let m1 = account_id_from_key(&key1);
    let m2 = account_id_from_key(&key2);
    let m3 = account_id_from_key(&key3);
    let recipient = account_id_from_key(&PrivateKey::new_os_random());

    // Create 2-of-3 multisig
    println!("🔐 Creating 2-of-3 multisig...");
    let instruction = Instruction::CreateMultisig {
        threshold: 2,
        members: vec![*m1.value(), *m2.value(), *m3.value()],
    };
    let message = Message::try_new(program_id, vec![multisig_state_id], vec![], instruction).unwrap();
    let witness_set = WitnessSet::for_message(&message, &[] as &[&PrivateKey]);
    submit_tx(&client, PublicTransaction::new(message, witness_set)).await;

    // Step 1: Member 1 proposes a transfer
    println!("📝 Member 1 proposing transfer...");
    let nonce = get_nonce(&client, m1).await;
    submit_signed(
        &client, program_id,
        vec![multisig_state_id, m1],
        &key1, vec![nonce],
        Instruction::Propose {
            action: ProposalAction::Transfer { recipient, amount: 100 },
        },
    ).await;

    // Verify proposal exists
    let state = get_multisig_state(&client, multisig_state_id).await;
    assert_eq!(state.proposals.len(), 1);
    assert_eq!(state.proposals[0].index, 1);
    assert_eq!(state.proposals[0].approved.len(), 1); // proposer auto-approved
    assert_eq!(state.proposals[0].status, ProposalStatus::Active);
    println!("  ✅ Proposal #1 created with 1 approval");

    // Step 2: Member 2 approves
    println!("👍 Member 2 approving...");
    let nonce = get_nonce(&client, m2).await;
    submit_signed(
        &client, program_id,
        vec![multisig_state_id, m2],
        &key2, vec![nonce],
        Instruction::Approve { proposal_index: 1 },
    ).await;

    let state = get_multisig_state(&client, multisig_state_id).await;
    assert_eq!(state.proposals[0].approved.len(), 2); // now at threshold
    println!("  ✅ Proposal #1 has 2 approvals (threshold reached!)");

    // Step 3: Member 1 executes
    println!("⚡ Member 1 executing...");
    let nonce = get_nonce(&client, m1).await;
    submit_signed(
        &client, program_id,
        vec![multisig_state_id, m1],
        &key1, vec![nonce],
        Instruction::Execute { proposal_index: 1 },
    ).await;

    let state = get_multisig_state(&client, multisig_state_id).await;
    // Executed proposals get cleaned up
    assert!(state.proposals.is_empty());
    println!("✅ Full propose → approve → execute flow completed!");
}

#[tokio::test]
async fn test_propose_reject() {
    let client = sequencer_client();
    let program_id = deploy_and_get_id(&client).await;
    let multisig_state_id = compute_multisig_state_pda(&program_id);

    let key1 = PrivateKey::new_os_random();
    let key2 = PrivateKey::new_os_random();
    let m1 = account_id_from_key(&key1);
    let m2 = account_id_from_key(&key2);

    // Create 2-of-2 multisig
    println!("🔐 Creating 2-of-2 multisig...");
    let instruction = Instruction::CreateMultisig {
        threshold: 2,
        members: vec![*m1.value(), *m2.value()],
    };
    let message = Message::try_new(program_id, vec![multisig_state_id], vec![], instruction).unwrap();
    let witness_set = WitnessSet::for_message(&message, &[] as &[&PrivateKey]);
    submit_tx(&client, PublicTransaction::new(message, witness_set)).await;

    // Member 1 proposes
    println!("📝 Member 1 proposing...");
    let nonce = get_nonce(&client, m1).await;
    submit_signed(
        &client, program_id,
        vec![multisig_state_id, m1],
        &key1, vec![nonce],
        Instruction::Propose {
            action: ProposalAction::Transfer {
                recipient: account_id_from_key(&PrivateKey::new_os_random()),
                amount: 100,
            },
        },
    ).await;

    // Member 2 rejects — in 2-of-2, one reject = dead proposal
    println!("👎 Member 2 rejecting...");
    let nonce = get_nonce(&client, m2).await;
    submit_signed(
        &client, program_id,
        vec![multisig_state_id, m2],
        &key2, vec![nonce],
        Instruction::Reject { proposal_index: 1 },
    ).await;

    let state = get_multisig_state(&client, multisig_state_id).await;
    let proposal = state.get_proposal(1).unwrap();
    assert_eq!(proposal.status, ProposalStatus::Rejected);
    println!("✅ Proposal correctly rejected!");
}

#[tokio::test]
async fn test_propose_add_member() {
    let client = sequencer_client();
    let program_id = deploy_and_get_id(&client).await;
    let multisig_state_id = compute_multisig_state_pda(&program_id);

    let key1 = PrivateKey::new_os_random();
    let key2 = PrivateKey::new_os_random();
    let m1 = account_id_from_key(&key1);
    let m2 = account_id_from_key(&key2);
    let new_member = account_id_from_key(&PrivateKey::new_os_random());

    // Create 1-of-2 multisig (so single approval suffices)
    println!("🔐 Creating 1-of-2 multisig...");
    let instruction = Instruction::CreateMultisig {
        threshold: 1,
        members: vec![*m1.value(), *m2.value()],
    };
    let message = Message::try_new(program_id, vec![multisig_state_id], vec![], instruction).unwrap();
    let witness_set = WitnessSet::for_message(&message, &[] as &[&PrivateKey]);
    submit_tx(&client, PublicTransaction::new(message, witness_set)).await;

    // Propose adding a member (auto-approves, threshold=1 so immediately ready)
    println!("📝 Proposing add member...");
    let nonce = get_nonce(&client, m1).await;
    submit_signed(
        &client, program_id,
        vec![multisig_state_id, m1],
        &key1, vec![nonce],
        Instruction::Propose {
            action: ProposalAction::AddMember { new_member: *new_member.value() },
        },
    ).await;

    // Execute immediately (threshold already met by proposer)
    println!("⚡ Executing add member...");
    let nonce = get_nonce(&client, m1).await;
    submit_signed(
        &client, program_id,
        vec![multisig_state_id, m1],
        &key1, vec![nonce],
        Instruction::Execute { proposal_index: 1 },
    ).await;

    let state = get_multisig_state(&client, multisig_state_id).await;
    assert_eq!(state.members.len(), 3);
    assert!(state.is_member(new_member.value()));
    println!("✅ Member added!");
}
