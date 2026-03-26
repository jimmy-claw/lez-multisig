pub mod create_multisig;
pub mod propose;
pub mod propose_config;
pub mod approve;
pub mod reject;
pub mod execute;

use nssa_core::program::ProgramId;
use multisig_core::ConfigAction;
use spel_framework::prelude::*;

/// Multisig program using #[lez_program] macro.
/// Uses external multisig_core::Instruction enum for dispatch.
///
/// LP-0002: propose/approve/reject use #[pre_tx_hook] for ZK-based private voting.
/// Members prove membership via vote_circuit ZK proofs instead of signer checks.
#[lez_program(instruction = "multisig_core::Instruction")]
mod multisig_program {
    use super::*;

    /// Create a new M-of-N multisig.
    /// multisig_state is initialized as a PDA derived from create_key.
    #[instruction]
    pub fn create_multisig(
        #[account(init, pda = arg("create_key"))]
        multisig_state: AccountWithMetadata,
        member_accounts: Vec<AccountWithMetadata>,
        create_key: [u8; 32],
        threshold: u8,
        members: Vec<[u8; 32]>,
    ) -> SpelResult {
        let accounts: Vec<AccountWithMetadata> = std::iter::once(multisig_state)
            .chain(member_accounts.into_iter())
            .collect();
        let (post_states, chained_calls) =
            crate::create_multisig::handle(&accounts, &create_key, threshold, &members);
        Ok(SpelOutput { post_states, chained_calls })
    }

    /// Propose a new transaction.
    /// Membership proven via ZK proof (vote_circuit).
    /// No signer account — the #[pre_tx_hook] generates and verifies the proof.
    /// proposal PDA seeds: ["multisig_prop___", create_key, proposal_index]
    #[instruction]
    #[pre_tx_hook(
        signer = "caller",
        elf = "vote_circuit.bin",
        inputs = [
            ("nsk", "bytes32", "wallet_nsk"),
            ("members", "vec_bytes32", "arg:members"),
            ("proposal_index", "u64", "arg:proposal_index"),
            ("domain", "string", "literal:propose"),
        ],
        outputs = [vote_receipt, nullifier]
    )]
    pub fn propose(
        #[account(mut, pda = arg("create_key"))]
        multisig_state: AccountWithMetadata,
        #[account(init, pda = [literal("multisig_prop___"), arg("create_key"), arg("proposal_index")])]
        proposal: AccountWithMetadata,
        target_program_id: ProgramId,
        target_instruction_data: Vec<u32>,
        target_account_count: u8,
        pda_seeds: Vec<[u8; 32]>,
        authorized_indices: Vec<u8>,
        create_key: [u8; 32],
        proposal_index: u64,
        members: Vec<[u8; 32]>,
        vote_receipt: Vec<u32>,
        nullifier: [u8; 32],
    ) -> SpelResult {
        let accounts = vec![multisig_state, proposal];
        let (post_states, chained_calls) = crate::propose::handle(
            &accounts,
            &target_program_id,
            &target_instruction_data,
            target_account_count,
            &pda_seeds,
            &authorized_indices,
            &vote_receipt,
            nullifier,
        );
        Ok(SpelOutput { post_states, chained_calls })
    }

    /// Approve an existing proposal.
    /// Membership proven via ZK proof (vote_circuit).
    /// proposal PDA seeds: ["multisig_prop___", create_key, proposal_index]
    #[instruction]
    #[pre_tx_hook(
        signer = "caller",
        elf = "vote_circuit.bin",
        inputs = [
            ("nsk", "bytes32", "wallet_nsk"),
            ("members", "vec_bytes32", "arg:members"),
            ("proposal_index", "u64", "arg:proposal_index"),
            ("domain", "string", "literal:approve"),
        ],
        outputs = [vote_receipt, nullifier]
    )]
    pub fn approve(
        #[account(mut)]
        multisig_state: AccountWithMetadata,
        #[account(mut, pda = [literal("multisig_prop___"), arg("create_key"), arg("proposal_index")])]
        proposal: AccountWithMetadata,
        proposal_index: u64,
        create_key: [u8; 32],
        members: Vec<[u8; 32]>,
        vote_receipt: Vec<u32>,
        nullifier: [u8; 32],
    ) -> SpelResult {
        let accounts = vec![multisig_state, proposal];
        let (post_states, chained_calls) =
            crate::approve::handle(&accounts, proposal_index, &vote_receipt, nullifier);
        Ok(SpelOutput { post_states, chained_calls })
    }

    /// Reject an existing proposal.
    /// Membership proven via ZK proof (vote_circuit).
    /// proposal PDA seeds: ["multisig_prop___", create_key, proposal_index]
    #[instruction]
    #[pre_tx_hook(
        signer = "caller",
        elf = "vote_circuit.bin",
        inputs = [
            ("nsk", "bytes32", "wallet_nsk"),
            ("members", "vec_bytes32", "arg:members"),
            ("proposal_index", "u64", "arg:proposal_index"),
            ("domain", "string", "literal:reject"),
        ],
        outputs = [vote_receipt, nullifier]
    )]
    pub fn reject(
        #[account(mut)]
        multisig_state: AccountWithMetadata,
        #[account(mut, pda = [literal("multisig_prop___"), arg("create_key"), arg("proposal_index")])]
        proposal: AccountWithMetadata,
        proposal_index: u64,
        create_key: [u8; 32],
        members: Vec<[u8; 32]>,
        vote_receipt: Vec<u32>,
        nullifier: [u8; 32],
    ) -> SpelResult {
        let accounts = vec![multisig_state, proposal];
        let (post_states, chained_calls) =
            crate::reject::handle(&accounts, proposal_index, &vote_receipt, nullifier);
        Ok(SpelOutput { post_states, chained_calls })
    }

    /// Execute a fully-approved proposal.
    /// No ZK proof needed — anyone can execute once threshold is met.
    /// proposal PDA seeds: ["multisig_prop___", create_key, proposal_index]
    #[instruction]
    pub fn execute(
        #[account(mut)]
        multisig_state: AccountWithMetadata,
        #[account(mut, pda = [literal("multisig_prop___"), arg("create_key"), arg("proposal_index")])]
        proposal: AccountWithMetadata,
        target_accounts: Vec<AccountWithMetadata>,
        proposal_index: u64,
        create_key: [u8; 32],
    ) -> SpelResult {
        let mut accounts = vec![multisig_state, proposal];
        accounts.extend(target_accounts);
        let (post_states, chained_calls) =
            crate::execute::handle(&accounts, proposal_index);
        Ok(SpelOutput { post_states, chained_calls })
    }

    /// Propose adding a new member.
    /// proposer must be a member signer. proposal is initialized.
    /// proposal PDA seeds: ["multisig_prop___", create_key, proposal_index]
    #[instruction]
    pub fn propose_add_member(
        #[account(mut)]
        multisig_state: AccountWithMetadata,
        #[account(signer)]
        proposer: AccountWithMetadata,
        #[account(init, pda = [literal("multisig_prop___"), arg("create_key"), arg("proposal_index")])]
        proposal: AccountWithMetadata,
        new_member: [u8; 32],
        create_key: [u8; 32],
        proposal_index: u64,
    ) -> SpelResult {
        let accounts = vec![multisig_state, proposer, proposal];
        let (post_states, chained_calls) = crate::propose_config::handle(
            &accounts,
            ConfigAction::AddMember { new_member },
        );
        Ok(SpelOutput { post_states, chained_calls })
    }

    /// Propose removing a member.
    /// proposer must be a member signer. proposal is initialized.
    /// proposal PDA seeds: ["multisig_prop___", create_key, proposal_index]
    #[instruction]
    pub fn propose_remove_member(
        #[account(mut)]
        multisig_state: AccountWithMetadata,
        #[account(signer)]
        proposer: AccountWithMetadata,
        #[account(init, pda = [literal("multisig_prop___"), arg("create_key"), arg("proposal_index")])]
        proposal: AccountWithMetadata,
        member: [u8; 32],
        create_key: [u8; 32],
        proposal_index: u64,
    ) -> SpelResult {
        let accounts = vec![multisig_state, proposer, proposal];
        let (post_states, chained_calls) = crate::propose_config::handle(
            &accounts,
            ConfigAction::RemoveMember { member },
        );
        Ok(SpelOutput { post_states, chained_calls })
    }

    /// Propose changing the threshold.
    /// proposer must be a member signer. proposal is initialized.
    /// proposal PDA seeds: ["multisig_prop___", create_key, proposal_index]
    #[instruction]
    pub fn propose_change_threshold(
        #[account(mut)]
        multisig_state: AccountWithMetadata,
        #[account(signer)]
        proposer: AccountWithMetadata,
        #[account(init, pda = [literal("multisig_prop___"), arg("create_key"), arg("proposal_index")])]
        proposal: AccountWithMetadata,
        new_threshold: u8,
        create_key: [u8; 32],
        proposal_index: u64,
    ) -> SpelResult {
        let accounts = vec![multisig_state, proposer, proposal];
        let (post_states, chained_calls) = crate::propose_config::handle(
            &accounts,
            ConfigAction::ChangeThreshold { new_threshold },
        );
        Ok(SpelOutput { post_states, chained_calls })
    }
}

// Legacy process() function for the existing guest binary.
// The #[lez_program] macro generates main() and IDL, but the guest binary
// (methods/guest/src/bin/multisig.rs) uses this for the risc0 entry point.
pub fn process(
    accounts: &[nssa_core::account::AccountWithMetadata],
    instruction: &multisig_core::Instruction,
) -> (Vec<nssa_core::program::AccountPostState>, Vec<nssa_core::program::ChainedCall>) {
    use multisig_core::Instruction;
    match instruction {
        Instruction::CreateMultisig { create_key, threshold, members } =>
            create_multisig::handle(accounts, create_key, *threshold, members),
        Instruction::Propose { target_program_id, target_instruction_data, target_account_count, pda_seeds, authorized_indices, vote_receipt, nullifier, .. } =>
            propose::handle(accounts, target_program_id, target_instruction_data, *target_account_count, pda_seeds, authorized_indices, vote_receipt, *nullifier),
        Instruction::Approve { proposal_index, vote_receipt, nullifier, .. } =>
            approve::handle(accounts, *proposal_index, vote_receipt, *nullifier),
        Instruction::Reject { proposal_index, vote_receipt, nullifier, .. } =>
            reject::handle(accounts, *proposal_index, vote_receipt, *nullifier),
        Instruction::Execute { proposal_index, .. } =>
            execute::handle(accounts, *proposal_index),
        Instruction::ProposeAddMember { new_member, .. } =>
            propose_config::handle(accounts, ConfigAction::AddMember { new_member: *new_member }),
        Instruction::ProposeRemoveMember { member, .. } =>
            propose_config::handle(accounts, ConfigAction::RemoveMember { member: *member }),
        Instruction::ProposeChangeThreshold { new_threshold, .. } =>
            propose_config::handle(accounts, ConfigAction::ChangeThreshold { new_threshold: *new_threshold }),
    }
}
