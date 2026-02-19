use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};
use nssa::{
    AccountId, PublicTransaction,
    program::Program,
    public_transaction::{Message, WitnessSet},
};
use multisig_core::{Instruction, ProposalAction, compute_multisig_state_pda};
use wallet::WalletCore;

/// LSSA Multisig CLI — M-of-N threshold governance for LEZ
///
/// Squads-style on-chain proposal flow:
///   propose → approve (by M members) → execute
#[derive(Parser)]
#[command(name = "multisig", version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    /// Path to the multisig program binary
    #[arg(long, short = 'p', env = "MULTISIG_PROGRAM", default_value = "target/riscv32im-risc0-zkvm-elf/docker/multisig.bin")]
    program: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new M-of-N multisig
    Create {
        /// Required signatures (M)
        #[arg(long, short = 't')]
        threshold: u8,
        /// Member account IDs (base58)
        #[arg(long, short = 'm', num_args = 1..)]
        member: Vec<String>,
    },

    /// Create a proposal for a multisig action
    Propose {
        /// Your account ID (base58, must be a member)
        #[arg(long)]
        account: String,

        #[command(subcommand)]
        action: ProposeAction,
    },

    /// Approve a proposal
    Approve {
        /// Proposal index
        #[arg(long, short = 'i')]
        index: u64,
        /// Your account ID (base58, must be a member)
        #[arg(long)]
        account: String,
    },

    /// Reject a proposal
    Reject {
        /// Proposal index
        #[arg(long, short = 'i')]
        index: u64,
        /// Your account ID (base58, must be a member)
        #[arg(long)]
        account: String,
    },

    /// Execute a fully-approved proposal
    Execute {
        /// Proposal index
        #[arg(long, short = 'i')]
        index: u64,
        /// Your account ID (base58, must be a member)
        #[arg(long)]
        account: String,
    },

    /// Show multisig status
    Status,

    /// Generate shell completions
    Completions {
        /// Shell to generate for
        #[arg(value_enum)]
        shell: Shell,
    },
}

#[derive(Subcommand)]
enum ProposeAction {
    /// Transfer funds from the multisig
    Transfer {
        /// Recipient account ID
        #[arg(long)]
        to: String,
        /// Amount to transfer
        #[arg(long)]
        amount: u128,
    },

    /// Add a new member
    AddMember {
        /// New member account ID
        #[arg(long)]
        member: String,
    },

    /// Remove a member
    RemoveMember {
        /// Member account ID to remove
        #[arg(long)]
        member: String,
    },

    /// Change the threshold
    SetThreshold {
        /// New threshold value
        #[arg(long, short = 't')]
        threshold: u8,
    },
}

fn load_program(path: &str) -> (Program, nssa::ProgramId) {
    let bytecode = std::fs::read(path)
        .unwrap_or_else(|e| {
            eprintln!("Error: Cannot read program binary at '{}': {}", path, e);
            eprintln!("  Build it first:  cargo risczero build --manifest-path methods/guest/Cargo.toml");
            eprintln!("  Or set path:     --program <path> or MULTISIG_PROGRAM=<path>");
            std::process::exit(1);
        });
    let program = Program::new(bytecode)
        .unwrap_or_else(|e| {
            eprintln!("Error: Invalid program bytecode at '{}': {:?}", path, e);
            std::process::exit(1);
        });
    let id = program.id();
    (program, id)
}

async fn submit_and_confirm(wallet_core: &WalletCore, tx: PublicTransaction, label: &str) {
    let response = wallet_core
        .sequencer_client
        .send_tx_public(tx)
        .await
        .unwrap();

    println!("📤 {} submitted", label);
    println!("   tx_hash: {}", response.tx_hash);
    println!("   Waiting for confirmation...");

    let poller = wallet::poller::TxPoller::new(
        wallet_core.config().clone(),
        wallet_core.sequencer_client.clone(),
    );

    match poller.poll_tx(response.tx_hash).await {
        Ok(_) => println!("✅ Confirmed!"),
        Err(e) => {
            eprintln!("❌ Not confirmed: {e:#}");
            std::process::exit(1);
        }
    }
}

/// Build and submit a single-signer transaction
async fn submit_signed_tx(
    wallet_core: &WalletCore,
    program_id: nssa::ProgramId,
    account_ids: Vec<AccountId>,
    signer_id: AccountId,
    instruction: Instruction,
    label: &str,
) {
    let nonces = wallet_core
        .get_accounts_nonces(vec![signer_id])
        .await
        .expect("Failed to get nonces");

    let signing_key = wallet_core
        .storage()
        .user_data
        .get_pub_account_signing_key(signer_id)
        .expect("Signing key not found — is this account in your wallet?");

    let message = Message::try_new(
        program_id,
        account_ids,
        nonces,
        instruction,
    ).unwrap();

    let witness_set = WitnessSet::for_message(&message, &[signing_key]);
    let tx = PublicTransaction::new(message, witness_set);
    submit_and_confirm(wallet_core, tx, label).await;
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Commands that don't need wallet/program
    match &cli.command {
        Commands::Completions { shell } => {
            generate(*shell, &mut Cli::command(), "multisig", &mut std::io::stdout());
            return;
        }
        Commands::Status => {
            println!("📊 Multisig Status");
            println!("   Program path:   {}", cli.program);
            if let Ok(bytecode) = std::fs::read(&cli.program) {
                if let Ok(program) = Program::new(bytecode) {
                    let program_id = program.id();
                    let multisig_state_id = compute_multisig_state_pda(&program_id);
                    println!("   Program ID:     {:?}", program_id);
                    println!("   Multisig PDA:   {}", multisig_state_id);
                }
            } else {
                println!("   Program binary: not found");
            }
            println!("   (On-chain state query not yet implemented)");
            return;
        }
        _ => {}
    }

    let wallet_core = WalletCore::from_env().unwrap();
    let (_, program_id) = load_program(&cli.program);
    let multisig_state_id = compute_multisig_state_pda(&program_id);

    match cli.command {
        // ── Create ──────────────────────────────────────────────────────
        Commands::Create { threshold, member } => {
            let members: Vec<AccountId> = member.iter()
                .map(|s| s.parse().expect("Invalid member ID"))
                .collect();

            if (threshold as usize) > members.len() {
                eprintln!("Error: threshold ({}) > members ({})", threshold, members.len());
                std::process::exit(1);
            }

            println!("🔐 Creating {}-of-{} multisig", threshold, members.len());
            println!("   State PDA:  {}", multisig_state_id);

            let instruction = Instruction::CreateMultisig {
                threshold,
                members: members.iter().map(|id| *id.value()).collect(),
            };

            let message = Message::try_new(
                program_id,
                vec![multisig_state_id],
                vec![],
                instruction,
            ).unwrap();
            let witness_set = WitnessSet::for_message(&message, &[] as &[&nssa::PrivateKey]);
            let tx = PublicTransaction::new(message, witness_set);
            submit_and_confirm(&wallet_core, tx, "Create multisig").await;
        }

        // ── Propose ─────────────────────────────────────────────────────
        Commands::Propose { account, action } => {
            let account_id: AccountId = account.parse().expect("Invalid account ID");

            let proposal_action = match &action {
                ProposeAction::Transfer { to, amount } => {
                    let to_id: AccountId = to.parse().expect("Invalid recipient ID");
                    ProposalAction::Transfer { recipient: to_id, amount: *amount }
                }
                ProposeAction::AddMember { member } => {
                    let member_id: AccountId = member.parse().expect("Invalid member ID");
                    ProposalAction::AddMember { new_member: *member_id.value() }
                }
                ProposeAction::RemoveMember { member } => {
                    let member_id: AccountId = member.parse().expect("Invalid member ID");
                    ProposalAction::RemoveMember { member_to_remove: *member_id.value() }
                }
                ProposeAction::SetThreshold { threshold } => {
                    ProposalAction::ChangeThreshold { new_threshold: *threshold }
                }
            };

            let instruction = Instruction::Propose { action: proposal_action };

            println!("📝 Creating proposal...");
            submit_signed_tx(
                &wallet_core, program_id,
                vec![multisig_state_id, account_id],
                account_id,
                instruction,
                "Propose",
            ).await;
        }

        // ── Approve ─────────────────────────────────────────────────────
        Commands::Approve { index, account } => {
            let account_id: AccountId = account.parse().expect("Invalid account ID");

            println!("👍 Approving proposal #{}...", index);
            submit_signed_tx(
                &wallet_core, program_id,
                vec![multisig_state_id, account_id],
                account_id,
                Instruction::Approve { proposal_index: index },
                "Approve",
            ).await;
        }

        // ── Reject ──────────────────────────────────────────────────────
        Commands::Reject { index, account } => {
            let account_id: AccountId = account.parse().expect("Invalid account ID");

            println!("👎 Rejecting proposal #{}...", index);
            submit_signed_tx(
                &wallet_core, program_id,
                vec![multisig_state_id, account_id],
                account_id,
                Instruction::Reject { proposal_index: index },
                "Reject",
            ).await;
        }

        // ── Execute ─────────────────────────────────────────────────────
        Commands::Execute { index, account } => {
            let account_id: AccountId = account.parse().expect("Invalid account ID");

            println!("⚡ Executing proposal #{}...", index);
            submit_signed_tx(
                &wallet_core, program_id,
                vec![multisig_state_id, account_id],
                account_id,
                Instruction::Execute { proposal_index: index },
                "Execute",
            ).await;
        }

        Commands::Completions { .. } | Commands::Status => unreachable!(),
    }
}
