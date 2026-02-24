use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};
use nssa::{
    AccountId, PublicTransaction,
    program::Program,
    public_transaction::{Message, WitnessSet},
};
use registry_core::{
    Instruction, RegisterArgs, UpdateArgs,
    compute_registry_state_pda, compute_program_entry_pda,
    ProgramEntry, RegistryState,
};
use wallet::WalletCore;

/// LEZ Program Registry CLI — on-chain registry for LEZ programs
///
/// Register your program with its IDL content hash (Codex CID) so tooling
/// and UIs can auto-discover callable programs and their interfaces.
///
/// Workflow:
///   1. Build and deploy your program to LEZ
///   2. Upload your IDL JSON to a Codex node → get CID
///   3. registry register --program-id <id> --name <name> --idl-cid <cid> ...
///   4. registry list / registry info <name>
#[derive(Parser)]
#[command(name = "registry", version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    /// Path to the registry program binary
    #[arg(
        long,
        short = 'p',
        env = "REGISTRY_PROGRAM",
        default_value = "target/riscv32im-risc0-zkvm-elf/docker/registry.bin"
    )]
    program: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Register a new program in the on-chain registry
    Register {
        /// Your account ID (base58) — becomes the author
        #[arg(long, short = 'a')]
        account: String,

        /// Program ID (hex-encoded [u32; 8], 64 hex chars)
        #[arg(long)]
        program_id: String,

        /// Human-readable name, e.g. "lez-multisig"
        #[arg(long, short = 'n')]
        name: String,

        /// Version string, e.g. "0.1.0"
        #[arg(long, short = 'v')]
        version: String,

        /// Codex CID of the IDL JSON on Logos Storage
        #[arg(long, short = 'c')]
        idl_cid: String,

        /// Free-form description
        #[arg(long, short = 'd', default_value = "")]
        description: String,

        /// Tags (repeat for multiple: --tag governance --tag multisig)
        #[arg(long, short = 't', num_args = 0..)]
        tag: Vec<String>,
    },

    /// Update metadata for an existing registered program
    Update {
        /// Your account ID (base58) — must be the original author
        #[arg(long, short = 'a')]
        account: String,

        /// Program ID (hex-encoded [u32; 8], 64 hex chars)
        #[arg(long)]
        program_id: String,

        /// New version string (empty = no change)
        #[arg(long, short = 'v', default_value = "")]
        version: String,

        /// New Codex CID (empty = no change)
        #[arg(long, short = 'c', default_value = "")]
        idl_cid: String,

        /// New description (empty = no change)
        #[arg(long, short = 'd', default_value = "")]
        description: String,

        /// New tags list — replaces existing (empty = no change)
        #[arg(long, short = 't', num_args = 0..)]
        tag: Vec<String>,
    },

    /// List all registered programs (queries registry state)
    List,

    /// Show details for one registered program by name
    Info {
        /// Program name to look up
        name: String,
    },

    /// Show registry status (program binary info)
    Status,

    /// Generate shell completions
    Completions {
        /// Shell to generate for
        #[arg(value_enum)]
        shell: Shell,
    },
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn load_program(path: &str) -> (Program, nssa::ProgramId) {
    let bytecode = std::fs::read(path).unwrap_or_else(|e| {
        eprintln!("Error: Cannot read registry program binary at '{}': {}", path, e);
        eprintln!(
            "  Build it first:  cargo risczero build --manifest-path methods/guest/Cargo.toml"
        );
        eprintln!("  Or set path:     --program <path> or REGISTRY_PROGRAM=<path>");
        std::process::exit(1);
    });
    let program = Program::new(bytecode).unwrap_or_else(|e| {
        eprintln!("Error: Invalid program bytecode at '{}': {:?}", path, e);
        std::process::exit(1);
    });
    let id = program.id();
    (program, id)
}

/// Parse a hex-encoded program ID string ("xxxxxxxx..." 64 hex chars → [u32; 8]).
fn parse_program_id(s: &str) -> nssa::ProgramId {
    let s = s.trim_start_matches("0x");
    if s.len() != 64 {
        eprintln!(
            "Error: program_id must be 64 hex characters (8 × u32 big-endian), got {} chars",
            s.len()
        );
        std::process::exit(1);
    }
    let bytes = hex::decode(s).unwrap_or_else(|e| {
        eprintln!("Error: invalid hex in program_id: {}", e);
        std::process::exit(1);
    });
    let mut pid = [0u32; 8];
    for (i, chunk) in bytes.chunks(4).enumerate() {
        pid[i] = u32::from_be_bytes(chunk.try_into().unwrap());
    }
    pid
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

    let message = Message::try_new(program_id, account_ids, nonces, instruction).unwrap();
    let witness_set = WitnessSet::for_message(&message, &[signing_key]);
    let tx = PublicTransaction::new(message, witness_set);
    submit_and_confirm(wallet_core, tx, label).await;
}

/// Fetch and deserialize an account's data as type T via the wallet.
async fn fetch_account_data<T: borsh::BorshDeserialize>(
    wallet_core: &WalletCore,
    account_id: AccountId,
    label: &str,
) -> Option<T> {
    let account = match wallet_core.get_account_public(account_id).await {
        Ok(acc) => acc,
        Err(e) => {
            eprintln!("Error fetching {}: {}", label, e);
            return None;
        }
    };
    let data: Vec<u8> = account.data.into();
    if data.is_empty() {
        return None;
    }
    borsh::from_slice::<T>(&data)
        .map(Some)
        .unwrap_or_else(|e| {
            eprintln!("Error deserializing {}: {}", label, e);
            std::process::exit(1);
        })
}

fn print_entry(entry: &ProgramEntry) {
    println!("  Name:          {}", entry.name);
    println!("  Version:       {}", entry.version);
    println!("  Author:        {}", entry.author);
    println!("  Program ID:    {:08x?}", entry.program_id);
    println!("  IDL CID:       {}", entry.idl_cid);
    println!("  Description:   {}", entry.description);
    println!("  Registered at: {}", entry.registered_at);
    if !entry.tags.is_empty() {
        println!("  Tags:          {}", entry.tags.join(", "));
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Commands that don't need wallet/program
    match &cli.command {
        Commands::Completions { shell } => {
            generate(
                *shell,
                &mut Cli::command(),
                "registry",
                &mut std::io::stdout(),
            );
            return;
        }
        Commands::Status => {
            println!("📋 Registry Program Status");
            println!("   Program path: {}", cli.program);
            if let Ok(bytecode) = std::fs::read(&cli.program) {
                if let Ok(program) = Program::new(bytecode) {
                    let id = program.id();
                    let registry_state_id = compute_registry_state_pda(&id);
                    println!("   Program ID:         {:08x?}", id);
                    println!("   Registry state PDA: {}", registry_state_id);
                }
            } else {
                println!("   Program binary: not found (build it first)");
            }
            return;
        }
        _ => {}
    }

    let wallet_core = WalletCore::from_env().unwrap();
    let (_, program_id) = load_program(&cli.program);

    match cli.command {
        // ── Register ────────────────────────────────────────────────────
        Commands::Register {
            account,
            program_id: prog_id_hex,
            name,
            version,
            idl_cid,
            description,
            tag,
        } => {
            let author_id: AccountId = account.parse().expect("Invalid account ID");
            let prog_id = parse_program_id(&prog_id_hex);

            let registry_state_id = compute_registry_state_pda(&program_id);
            let entry_pda_id = compute_program_entry_pda(&program_id, &prog_id);

            println!("📝 Registering program '{}' v{}", name, version);
            println!("   Registry state: {}", registry_state_id);
            println!("   Entry PDA:      {}", entry_pda_id);
            println!("   IDL CID:        {}", idl_cid);

            let instruction = Instruction::Register(RegisterArgs {
                program_id: prog_id,
                name: name.clone(),
                version: version.clone(),
                idl_cid: idl_cid.clone(),
                description,
                tags: tag,
            });

            submit_signed_tx(
                &wallet_core,
                program_id,
                vec![registry_state_id, author_id, entry_pda_id],
                author_id,
                instruction,
                &format!("Register '{}'", name),
            )
            .await;

            println!("\n✅ Program '{}' registered successfully!", name);
            println!("   Entry PDA: {}", entry_pda_id);
        }

        // ── Update ──────────────────────────────────────────────────────
        Commands::Update {
            account,
            program_id: prog_id_hex,
            version,
            idl_cid,
            description,
            tag,
        } => {
            let author_id: AccountId = account.parse().expect("Invalid account ID");
            let prog_id = parse_program_id(&prog_id_hex);

            let registry_state_id = compute_registry_state_pda(&program_id);
            let entry_pda_id = compute_program_entry_pda(&program_id, &prog_id);

            println!("🔄 Updating program entry...");
            println!("   Entry PDA: {}", entry_pda_id);

            let instruction = Instruction::Update(UpdateArgs {
                program_id: prog_id,
                version,
                idl_cid,
                description,
                tags: tag,
            });

            submit_signed_tx(
                &wallet_core,
                program_id,
                vec![registry_state_id, author_id, entry_pda_id],
                author_id,
                instruction,
                "Update program entry",
            )
            .await;

            println!("\n✅ Program entry updated successfully!");
        }

        // ── List ────────────────────────────────────────────────────────
        Commands::List => {
            let registry_state_id = compute_registry_state_pda(&program_id);

            println!("📋 Fetching registry state...");

            let state: Option<RegistryState> =
                fetch_account_data(&wallet_core, registry_state_id, "registry state").await;

            match state {
                None => {
                    println!("Registry not yet initialized (no programs registered).");
                }
                Some(s) => {
                    println!("📦 Registry — {} program(s) registered", s.program_count);
                    println!();
                    println!("ℹ️  To look up a specific program use: registry info <name>");
                    println!("   (Enumeration of all entries requires off-chain indexing in v1)");
                }
            }
        }

        // ── Info ────────────────────────────────────────────────────────
        Commands::Info { name } => {
            // NOTE: In v1, PDA derivation is by program_id (hash), not by name.
            // The `info` command is a best-effort lookup — the user must provide
            // the program_id hex. For discovery by name, an off-chain indexer is
            // needed. This command is a placeholder that demonstrates the flow.
            eprintln!("ℹ️  'registry info' requires the program_id hex to derive the PDA.");
            eprintln!("   Usage: registry info <program_id_hex>");
            eprintln!("   (name-based lookup requires an off-chain indexer — coming in v2)");
            eprintln!();
            eprintln!("   Looking up name: '{}'", name);

            // For now, treat `name` as a hex program_id if it looks like one,
            // otherwise print a helpful message.
            if name.len() == 64 || name.starts_with("0x") {
                let prog_id = parse_program_id(&name);
                let entry_pda_id = compute_program_entry_pda(&program_id, &prog_id);
                println!("   Entry PDA: {}", entry_pda_id);

                let entry: Option<ProgramEntry> =
                    fetch_account_data(&wallet_core, entry_pda_id, "program entry").await;

                match entry {
                    None => println!("   No program entry found for this ID."),
                    Some(e) => {
                        println!();
                        print_entry(&e);
                    }
                }
            } else {
                eprintln!("   Pass a 64-char hex program_id to look up by ID.");
                std::process::exit(1);
            }
        }

        Commands::Completions { .. } | Commands::Status => unreachable!(),
    }
}
