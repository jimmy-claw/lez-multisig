// ChangeThreshold handler — changes the M-of-N threshold (requires threshold signatures)

use nssa_core::account::AccountWithMetadata;
use nssa_core::program::{AccountPostState, ChainedCall};
use multisig_core::MultisigState;

/// Handle ChangeThreshold instruction
/// 
/// Expected accounts:
/// - accounts[0]: multisig_state (PDA) — contains threshold, members, nonce
/// - accounts[1..]: authorized accounts — the signers (must have is_authorized = true)
/// 
/// Authorization: M distinct members must be authorized
pub fn handle(
    accounts: &[AccountWithMetadata],
    new_threshold: u8,
) -> (Vec<AccountPostState>, Vec<ChainedCall>) {
    // Parse accounts
    assert!(accounts.len() >= 2, "ChangeThreshold requires multisig_state and authorized accounts");
    
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
    
    // Validate new threshold
    assert!(new_threshold >= 1, "Threshold must be at least 1");
    assert!(
        new_threshold <= state.member_count,
        "Threshold cannot exceed member count"
    );
    
    // Update threshold
    state.threshold = new_threshold;
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
    fn test_change_threshold_threshold_met() {
        let members = vec![[1u8; 32], [2u8; 32], [3u8; 32]];
        let state_data = make_multisig_state(2, members);
        
        let mut acc1 = make_account(&[1u8; 32], vec![]);
        acc1.is_authorized = true;
        let mut acc2 = make_account(&[2u8; 32], vec![]);
        acc2.is_authorized = true;
        
        let accounts = vec![
            make_account(&[10u8; 32], state_data),
            acc1,
            acc2,
        ];
        
        let (post_states, _) = handle(&accounts, 3); // Change to threshold 3
        
        assert_eq!(post_states.len(), 1);
        let state_data: Vec<u8> = post_states[0].account().data.clone().into();
        let state: MultisigState = borsh::from_slice(&state_data).unwrap();
        assert_eq!(state.threshold, 3);
        assert_eq!(state.nonce, 1);
    }

    #[test]
    #[should_panic(expected = "Insufficient signatures")]
    fn test_change_threshold_not_met() {
        let members = vec![[1u8; 32], [2u8; 32], [3u8; 32]];
        let state_data = make_multisig_state(2, members);
        
        let mut acc1 = make_account(&[1u8; 32], vec![]);
        acc1.is_authorized = true;
        
        let accounts = vec![
            make_account(&[10u8; 32], state_data),
            acc1,
        ];
        
        handle(&accounts, 3);
    }

    #[test]
    #[should_panic(expected = "Threshold must be at least 1")]
    fn test_change_threshold_zero() {
        let members = vec![[1u8; 32], [2u8; 32]];
        let state_data = make_multisig_state(1, members);
        
        let mut acc1 = make_account(&[1u8; 32], vec![]);
        acc1.is_authorized = true;
        
        let accounts = vec![
            make_account(&[10u8; 32], state_data),
            acc1,
        ];
        
        handle(&accounts, 0);
    }

    #[test]
    #[should_panic(expected = "Threshold cannot exceed member count")]
    fn test_change_threshold_exceeds_members() {
        let members = vec![[1u8; 32], [2u8; 32]];
        let state_data = make_multisig_state(1, members);
        
        let mut acc1 = make_account(&[1u8; 32], vec![]);
        acc1.is_authorized = true;
        
        let accounts = vec![
            make_account(&[10u8; 32], state_data),
            acc1,
        ];
        
        handle(&accounts, 5); // 5 > 2 members
    }
}
