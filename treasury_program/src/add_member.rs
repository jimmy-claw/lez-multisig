// AddMember handler — adds a new member (requires threshold signatures)

use nssa_core::account::AccountWithMetadata;
use nssa_core::program::{AccountPostState, ChainedCall};
use treasury_core::MultisigState;

/// Handle AddMember instruction
/// 
/// Expected accounts:
/// - accounts[0]: multisig_state (PDA) — contains threshold, members, nonce
/// - accounts[1..]: authorized accounts — the signers (must have is_authorized = true)
/// 
/// Authorization: M distinct members must be authorized
pub fn handle(
    accounts: &[AccountWithMetadata],
    new_member: &[u8; 32],
) -> (Vec<AccountPostState>, Vec<ChainedCall>) {
    // Parse accounts
    assert!(accounts.len() >= 2, "AddMember requires multisig_state and authorized accounts");
    
    let multisig_account = &accounts[0];
    
    // Get authorized signers
    let authorized_signers: Vec<[u8; 32]> = accounts[1..]
        .iter()
        .filter(|acc| acc.is_authorized)
        .map(|acc| {
            let id_bytes: Vec<u8> = acc.account_id.value().clone().into();
            let mut key = [0u8; 32];
            key.copy_from_slice(&id_bytes[..32]);
            key
        })
        .collect();
    
    // Deserialize multisig state
    let state_data: Vec<u8> = multisig_account.account.data.clone().into();
    let mut state: MultisigState = borsh::from_slice(&state_data)
        .expect("Failed to deserialize multisig state");
    
    // Check threshold
    let valid_signers = state.count_valid_signers(&authorized_signers);
    assert!(
        valid_signers >= state.threshold as usize,
        "Insufficient signatures: need {}, got {}",
        state.threshold,
        valid_signers
    );
    
    // Check member not already exists
    assert!(!state.is_member(new_member), "Member already exists");
    
    // Check member limit
    assert!(state.member_count < 10, "Maximum 10 members for PoC");
    
    // Add member
    state.members.push(*new_member);
    state.member_count += 1;
    state.nonce += 1;
    
    // Build post state
    let mut multisig_post = multisig_account.account.clone();
    let state_bytes = borsh::to_vec(&state).unwrap();
    multisig_post.data = state_bytes.try_into().unwrap();

    (vec![AccountPostState::new(multisig_post)], vec![])
}

#[cfg(test)]
mod tests {
    use super::*;
    use nssa_core::account::{Account, AccountId};

    fn make_account(id: &[u8; 32], data: Vec<u8>) -> AccountWithMetadata {
        let mut account = Account::default();
        account.data = data.try_into().unwrap();
        AccountWithMetadata {
            account_id: AccountId::new(*id),
            account,
            is_authorized: false,
        }
    }

    fn make_multisig_state(threshold: u8, members: Vec<[u8; 32]>) -> Vec<u8> {
        let state = MultisigState::new(threshold, members);
        borsh::to_vec(&state).unwrap()
    }

    #[test]
    fn test_add_member_threshold_met() {
        let members = vec![[1u8; 32], [2u8; 32], [3u8; 32]];
        let state_data = make_multisig_state(2, members.clone());
        
        // 2 signers for threshold 2
        let mut acc1 = make_account(&[1u8; 32], vec![]);
        acc1.is_authorized = true;
        let mut acc2 = make_account(&[2u8; 32], vec![]);
        acc2.is_authorized = true;
        
        let accounts = vec![
            make_account(&[10u8; 32], state_data),
            acc1,
            acc2,
        ];
        
        let new_member = [4u8; 32];
        let (post_states, _) = handle(&accounts, &new_member);
        
        assert_eq!(post_states.len(), 1);
        let state_data: Vec<u8> = post_states[0].account().data.clone().into();
        let state: MultisigState = borsh::from_slice(&state_data).unwrap();
        assert_eq!(state.member_count, 4);
        assert_eq!(state.nonce, 1);
    }

    #[test]
    #[should_panic(expected = "Insufficient signatures")]
    fn test_add_member_threshold_not_met() {
        let members = vec![[1u8; 32], [2u8; 32], [3u8; 32]];
        let state_data = make_multisig_state(2, members);
        
        // Only 1 signer for threshold 2
        let mut acc1 = make_account(&[1u8; 32], vec![]);
        acc1.is_authorized = true;
        
        let accounts = vec![
            make_account(&[10u8; 32], state_data),
            acc1,
        ];
        
        let new_member = [4u8; 32];
        handle(&accounts, &new_member);
    }

    #[test]
    #[should_panic(expected = "Member already exists")]
    fn test_add_member_already_exists() {
        let members = vec![[1u8; 32], [2u8; 32]];
        let state_data = make_multisig_state(1, members);
        
        let mut acc1 = make_account(&[1u8; 32], vec![]);
        acc1.is_authorized = true;
        
        let accounts = vec![
            make_account(&[10u8; 32], state_data),
            acc1,
        ];
        
        // Try to add member 1 (already exists)
        handle(&accounts, &[1u8; 32]);
    }

    #[test]
    fn test_add_member_increments_nonce() {
        let members = vec![[1u8; 32], [2u8; 32]];
        let state_data = make_multisig_state(1, members);
        
        let mut acc1 = make_account(&[1u8; 32], vec![]);
        acc1.is_authorized = true;
        
        let accounts = vec![
            make_account(&[10u8; 32], state_data),
            acc1,
        ];
        
        let new_member = [3u8; 32];
        let (post_states, _) = handle(&accounts, &new_member);
        
        let state_data: Vec<u8> = post_states[0].account().data.clone().into();
        let state: MultisigState = borsh::from_slice(&state_data).unwrap();
        assert_eq!(state.nonce, 1);
    }

    #[test]
    fn test_add_member_different_member_not_authorized() {
        // Non-member signing should be ignored, but threshold still met by member
        let members = vec![[1u8; 32], [2u8; 32], [3u8; 32]];
        let state_data = make_multisig_state(1, members); // threshold 1
        
        // Member 1 is authorized, but member 9 is not a member (should be ignored)
        let mut acc1 = make_account(&[1u8; 32], vec![]);
        acc1.is_authorized = true;
        let mut acc9 = make_account(&[9u8; 32], vec![]);
        acc9.is_authorized = true; // Signs but not a member
        
        let accounts = vec![
            make_account(&[10u8; 32], state_data),
            acc1,
            acc9,
        ];
        
        let new_member = [4u8; 32];
        let (post_states, _) = handle(&accounts, &new_member);
        
        let state_data: Vec<u8> = post_states[0].account().data.clone().into();
        let state: MultisigState = borsh::from_slice(&state_data).unwrap();
        assert_eq!(state.member_count, 4);
    }
}
