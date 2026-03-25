use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};
use nssa::{
    AccountId, PublicTransaction,
    program::Program,
    public_transaction::{Message, WitnessSet},
};
use nssa_core::NullifierPublicKey;
use multisig_core::{
    Instruction,
    compute_multisig_state_pda,
    compute_proposal_pda,
};
use multisig_client::VoteProver;
use wallet::WalletCore;

/// LSSA Private Multisig CLI — M-of-N threshold governance with ZK voting
///
/// Privacy-preserving proposal flow:
///   propose → approve (by M members) → execute
///
/// Membership is proven via ZK proof (vote_circuit). The voter's identity
/// (NSK) never leaves the client — only the nullifier appears on-chain.
#[derive(Parser)]
#[command(name = "multisig", version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    /// Path to the multisig program binary
    #[arg(long, short = 'p', env = "MULTISIG_PROGRAM", default_value = "target/riscv32im-risc0-zkvm-elf/docker/multisig.bin")]
    program: String,

    /// Don't wait for transaction confirmation (print tx_hash and exit)
    #[arg(long, default_value_t = false)]
    no_wait: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new M-of-N multisig with NPK-based membership
    Create {
        /// Required signatures (M)
        #[arg(long, short = 't')]
        threshold: u8,
        /// Member nullifier public keys (hex-encoded, 64 chars each)
        #[arg(long, short = 'm', num_args = 1..)]
        members: Vec<String>,
        /// Optional create key (base58). If omitted, a random one is generated.
        #[arg(long)]
        create_key: Option<String>,
    },

    /// Create a proposal (any member, proven via ZK)
    Propose {
        /// Multisig create_key (base58)
        #[arg(long)]
        multisig: String,
        /// Your nullifier secret key (hex, 64 chars). Never sent on-chain.
        #[arg(long)]
        nsk: String,
        /// Member NPKs (hex). Required until on-chain state fetching is implemented.
        #[arg(long, num_args = 1..)]
        members: Vec<String>,
        /// Target program ID (hex, 64 chars)
        #[arg(long)]
        target_program: String,
        /// Serialized instruction data for the target program (hex u32 words or decimal)
        #[arg(long, num_args = 0..)]
        instruction_data: Vec<String>,
        /// Number of target accounts expected at execute time
        #[arg(long, default_value = "0")]
        target_account_count: u8,
        /// PDA seeds (hex-encoded 32-byte values)
        #[arg(long, num_args = 0..)]
        pda_seed: Vec<String>,
        /// Which target account indices (0-based) get is_authorized=true
        #[arg(long, num_args = 0..)]
        authorized_index: Vec<u8>,
        /// Proposal index hint (used to compute proposal PDA)
        #[arg(long)]
        proposal_index: u64,
    },

    /// Approve a proposal (ZK-proven membership, anonymous vote)
    Approve {
        /// Multisig create_key (base58)
        #[arg(long)]
        multisig: String,
        /// Proposal index
        #[arg(long)]
        proposal: u64,
        /// Your nullifier secret key (hex, 64 chars). Never sent on-chain.
        #[arg(long)]
        nsk: String,
        /// Member NPKs (hex). Required until on-chain state fetching is implemented.
        #[arg(long, num_args = 1..)]
        members: Vec<String>,
    },

    /// Reject a proposal (ZK-proven membership, anonymous vote)
    Reject {
        /// Multisig create_key (base58)
        #[arg(long)]
        multisig: String,
        /// Proposal index
        #[arg(long)]
        proposal: u64,
        /// Your nullifier secret key (hex, 64 chars). Never sent on-chain.
        #[arg(long)]
        nsk: String,
        /// Member NPKs (hex). Required until on-chain state fetching is implemented.
        #[arg(long, num_args = 1..)]
        members: Vec<String>,
    },

    /// Execute a fully-approved proposal (ZK-proven membership)
    Execute {
        /// Multisig create_key (base58)
        #[arg(long)]
        multisig: String,
        /// Proposal index
        #[arg(long)]
        proposal: u64,
        /// Your nullifier secret key (hex, 64 chars). Never sent on-chain.
        #[arg(long)]
        nsk: String,
        /// Member NPKs (hex). Required until on-chain state fetching is implemented.
        #[arg(long, num_args = 1..)]
        members: Vec<String>,
    },

    /// Show multisig status
    Status {
        /// Multisig create_key (base58). If omitted, shows general info.
        #[arg(long)]
        multisig: Option<String>,
    },

    /// Derive NPK (nullifier public key) from an NSK (nullifier secret key)
    DeriveNpk {
        /// NSK hex (64 chars)
        #[arg(long)]
        nsk: String,
    },

    /// Generate shell completions
    Completions {
        /// Shell to generate for
        #[arg(value_enum)]
        shell: Shell,
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

async fn submit_and_confirm(wallet_core: &WalletCore, tx: PublicTransaction, _label: &str, no_wait: bool) {
    let response = wallet_core
        .sequencer_client
        .send_tx_public(tx)
        .await
        .unwrap();

    println!("   tx_hash: {}", response.tx_hash);

    if no_wait {
        println!("   Submitted (--no-wait, skipping confirmation)");
        return;
    }

    println!("   Waiting for confirmation...");

    let poller = wallet::poller::TxPoller::new(
        wallet_core.config().clone(),
        wallet_core.sequencer_client.clone(),
    );

    match poller.poll_tx(response.tx_hash).await {
        Ok(_) => println!("   Confirmed!"),
        Err(e) => {
            eprintln!("   Not confirmed: {e:#}");
            std::process::exit(1);
        }
    }
}

/// Submit an unsigned transaction (no signer needed — membership proven via ZK).
async fn submit_private_tx(
    wallet_core: &WalletCore,
    program_id: nssa::ProgramId,
    account_ids: Vec<AccountId>,
    instruction: Instruction,
    label: &str,
    no_wait: bool,
) {
    let message = Message::try_new(
        program_id,
        account_ids,
        vec![],
        instruction,
    ).unwrap();
    let witness_set = WitnessSet::for_message(&message, &[] as &[&nssa::PrivateKey]);
    let tx = PublicTransaction::new(message, witness_set);
    submit_and_confirm(wallet_core, tx, label, no_wait).await;
}

/// Parse a hex string into a 32-byte array.
fn parse_hex32(s: &str) -> [u8; 32] {
    let bytes = hex::decode(s).expect("Invalid hex value (expected 64 hex chars for 32 bytes)");
    if bytes.len() != 32 {
        eprintln!("Error: expected 32 bytes (64 hex chars), got {}", bytes.len());
        std::process::exit(1);
    }
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    arr
}

/// Parse create_key from base58 AccountId string to [u8; 32].
fn parse_create_key(s: &str) -> [u8; 32] {
    let id: AccountId = s.parse().expect("Invalid multisig create_key (base58)");
    *id.value()
}

/// Parse member NPK hex strings into NullifierPublicKeys.
fn parse_member_npks(hex_strings: &[String]) -> Vec<NullifierPublicKey> {
    hex_strings.iter()
        .map(|s| NullifierPublicKey(parse_hex32(s)))
        .collect()
}

/// Parse a ProgramId ([u32; 8]) from a 64-char hex string (32 bytes, interpreted as 8 little-endian u32s).
fn parse_program_id(s: &str) -> nssa::ProgramId {
    let bytes = hex::decode(s).unwrap_or_else(|_| {
        eprintln!("Error: invalid hex for program ID (expected 64 hex chars): {}", s);
        std::process::exit(1);
    });
    if bytes.len() != 32 {
        eprintln!("Error: program ID must be 32 bytes (64 hex chars), got {}", bytes.len());
        std::process::exit(1);
    }
    let mut id = [0u32; 8];
    for i in 0..8 {
        id[i] = u32::from_le_bytes([bytes[i*4], bytes[i*4+1], bytes[i*4+2], bytes[i*4+3]]);
    }
    id
}

/// Parse hex-encoded u32 words into Vec<u32>.
fn parse_instruction_data(args: &[String]) -> Vec<u32> {
    args.iter().map(|s| {
        if let Ok(bytes) = hex::decode(s) {
            if bytes.len() == 4 {
                return u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            }
        }
        s.parse::<u32>().unwrap_or_else(|_| {
            eprintln!("Error: instruction data word '{}' is neither valid 4-byte hex nor decimal u32", s);
            std::process::exit(1);
        })
    }).collect()
}

/// Generate a ZK vote proof using the VoteProver.
fn generate_vote_proof(
    nsk_hex: &str,
    member_npks: &[NullifierPublicKey],
    proposal_index: u64,
    domain: &str,
) -> (Vec<u32>, [u8; 32]) {
    let nsk = parse_hex32(nsk_hex);
    let prover = VoteProver::new(member_npks.to_vec());
    prover.generate_proof(&nsk, proposal_index, domain)
        .unwrap_or_else(|e| {
            eprintln!("Error generating ZK proof: {e}");
            std::process::exit(1);
        })
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Commands that don't need wallet/program
    match &cli.command {
        Commands::DeriveNpk { nsk } => {
            let nsk_bytes = parse_hex32(nsk);
            let npk = NullifierPublicKey::from(&nsk_bytes);
            println!("{}", hex::encode(npk.0));
            return;
        }
        Commands::Completions { shell } => {
            generate(*shell, &mut Cli::command(), "multisig", &mut std::io::stdout());
            return;
        }
        Commands::Status { multisig } => {
            println!("Multisig Status");
            println!("   Program path: {}", cli.program);
            if let Ok(bytecode) = std::fs::read(&cli.program) {
                if let Ok(program) = Program::new(bytecode) {
                    println!("   Program ID:   {:?}", program.id());
                }
            } else {
                println!("   Program binary: not found");
            }
            if let Some(ms) = multisig {
                let ck = parse_create_key(ms);
                // Load program to compute PDA
                if let Ok(bytecode) = std::fs::read(&cli.program) {
                    if let Ok(program) = Program::new(bytecode) {
                        let pid = program.id();
                        let state_pda = compute_multisig_state_pda(&pid, &ck);
                        println!("   Create key:   {}", AccountId::new(ck));
                        println!("   State PDA:    {}", state_pda);
                        // TODO: fetch and display on-chain MultisigState
                    }
                }
            }
            return;
        }
        _ => {}
    }

    let wallet_core = WalletCore::from_env().unwrap();
    let (_, program_id) = load_program(&cli.program);

    match cli.command {
        // ── Create ──────────────────────────────────────────────────────
        //
        // Account layout: [state_pda]
        // No signer required — anyone can create.
        Commands::Create { threshold, members, create_key } => {
            let member_npks = parse_member_npks(&members);

            if (threshold as usize) > member_npks.len() {
                eprintln!("Error: threshold ({}) > members ({})", threshold, member_npks.len());
                std::process::exit(1);
            }

            // Generate or use provided create_key
            let ck: [u8; 32] = if let Some(ref key_str) = create_key {
                parse_create_key(key_str)
            } else {
                let random_key = nssa::PrivateKey::new_os_random();
                let pk = nssa::PublicKey::new_from_private_key(&random_key);
                *AccountId::from(&pk).value()
            };

            let multisig_state_id = compute_multisig_state_pda(&program_id, &ck);

            println!("Creating {}-of-{} multisig", threshold, member_npks.len());
            println!("   Create key: {}", AccountId::new(ck));
            println!("   State PDA:  {}", multisig_state_id);

            let instruction = Instruction::CreateMultisig {
                create_key: ck,
                threshold,
                members: member_npks,
            };

            let account_ids = vec![multisig_state_id];

            let message = Message::try_new(
                program_id,
                account_ids,
                vec![],
                instruction,
            ).unwrap();
            let witness_set = WitnessSet::for_message(&message, &[] as &[&nssa::PrivateKey]);
            let tx = PublicTransaction::new(message, witness_set);
            submit_and_confirm(&wallet_core, tx, "Create multisig", cli.no_wait).await;

            println!("\n   Save this create key to interact with the multisig:");
            println!("   {}", AccountId::new(ck));
        }

        // ── Propose ─────────────────────────────────────────────────────
        //
        // Account layout: [state_pda, proposal_pda]
        // No signer — membership proven via ZK proof.
        Commands::Propose {
            multisig,
            nsk,
            members,
            target_program,
            instruction_data,
            target_account_count,
            pda_seed,
            authorized_index,
            proposal_index,
        } => {
            let ck = parse_create_key(&multisig);
            let multisig_state_id = compute_multisig_state_pda(&program_id, &ck);
            let proposal_pda = compute_proposal_pda(&program_id, &ck, proposal_index);
            let member_npks = parse_member_npks(&members);

            let target_program_id: nssa::ProgramId = parse_program_id(&target_program);
            let target_instruction_data = parse_instruction_data(&instruction_data);
            let pda_seeds: Vec<[u8; 32]> = pda_seed.iter()
                .map(|s| parse_hex32(s))
                .collect();

            println!("Generating ZK proof for propose...");
            let (vote_receipt, nullifier) = generate_vote_proof(
                &nsk, &member_npks, proposal_index, "propose",
            );

            println!("Creating proposal #{}...", proposal_index);
            println!("   State PDA:    {}", multisig_state_id);
            println!("   Proposal PDA: {}", proposal_pda);
            println!("   Nullifier:    {}", hex::encode(nullifier));

            let instruction = Instruction::Propose {
                target_program_id,
                target_instruction_data,
                target_account_count,
                pda_seeds,
                authorized_indices: authorized_index,
                vote_receipt,
                nullifier,
            };

            submit_private_tx(
                &wallet_core, program_id,
                vec![multisig_state_id, proposal_pda],
                instruction,
                "Propose",
                cli.no_wait,
            ).await;
        }

        // ── Approve ─────────────────────────────────────────────────────
        //
        // Account layout: [state_pda, proposal_pda]
        // No signer — membership proven via ZK proof.
        Commands::Approve { multisig, proposal, nsk, members } => {
            let ck = parse_create_key(&multisig);
            let multisig_state_id = compute_multisig_state_pda(&program_id, &ck);
            let proposal_pda = compute_proposal_pda(&program_id, &ck, proposal);
            let member_npks = parse_member_npks(&members);

            println!("Generating ZK proof for approve...");
            let (vote_receipt, nullifier) = generate_vote_proof(
                &nsk, &member_npks, proposal, "approve",
            );

            println!("Approving proposal #{}...", proposal);
            println!("   State PDA:    {}", multisig_state_id);
            println!("   Proposal PDA: {}", proposal_pda);
            println!("   Nullifier:    {}", hex::encode(nullifier));

            submit_private_tx(
                &wallet_core, program_id,
                vec![multisig_state_id, proposal_pda],
                Instruction::Approve {
                    proposal_index: proposal,
                    vote_receipt,
                    nullifier,
                },
                "Approve",
                cli.no_wait,
            ).await;
        }

        // ── Reject ──────────────────────────────────────────────────────
        //
        // Account layout: [state_pda, proposal_pda]
        // No signer — membership proven via ZK proof.
        Commands::Reject { multisig, proposal, nsk, members } => {
            let ck = parse_create_key(&multisig);
            let multisig_state_id = compute_multisig_state_pda(&program_id, &ck);
            let proposal_pda = compute_proposal_pda(&program_id, &ck, proposal);
            let member_npks = parse_member_npks(&members);

            println!("Generating ZK proof for reject...");
            let (vote_receipt, nullifier) = generate_vote_proof(
                &nsk, &member_npks, proposal, "reject",
            );

            println!("Rejecting proposal #{}...", proposal);
            println!("   State PDA:    {}", multisig_state_id);
            println!("   Proposal PDA: {}", proposal_pda);
            println!("   Nullifier:    {}", hex::encode(nullifier));

            submit_private_tx(
                &wallet_core, program_id,
                vec![multisig_state_id, proposal_pda],
                Instruction::Reject {
                    proposal_index: proposal,
                    vote_receipt,
                    nullifier,
                },
                "Reject",
                cli.no_wait,
            ).await;
        }

        // ── Execute ─────────────────────────────────────────────────────
        //
        // Account layout: [state_pda, proposal_pda]
        // No signer — membership proven via ZK proof.
        // Target accounts are handled by ChainedCall inside the program.
        Commands::Execute { multisig, proposal, nsk, members } => {
            let ck = parse_create_key(&multisig);
            let multisig_state_id = compute_multisig_state_pda(&program_id, &ck);
            let proposal_pda = compute_proposal_pda(&program_id, &ck, proposal);
            let member_npks = parse_member_npks(&members);

            println!("Generating ZK proof for execute...");
            let (vote_receipt, nullifier) = generate_vote_proof(
                &nsk, &member_npks, proposal, "execute",
            );

            println!("Executing proposal #{}...", proposal);
            println!("   State PDA:    {}", multisig_state_id);
            println!("   Proposal PDA: {}", proposal_pda);
            println!("   Nullifier:    {}", hex::encode(nullifier));

            submit_private_tx(
                &wallet_core, program_id,
                vec![multisig_state_id, proposal_pda],
                Instruction::Execute {
                    proposal_index: proposal,
                    vote_receipt,
                    nullifier,
                },
                "Execute",
                cli.no_wait,
            ).await;
        }

        Commands::Completions { .. } | Commands::Status { .. } | Commands::DeriveNpk { .. } => unreachable!(),
    }
}
