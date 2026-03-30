use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let workspace_root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .unwrap()
        .to_path_buf();

    let elf_path = workspace_root.join(
        "target/riscv-guest/multisig-methods/multisig-guest/riscv32im-risc0-zkvm-elf/release/vote-circuit.bin",
    );

    println!("cargo:rerun-if-changed={}", elf_path.display());

    let elf_bytes = fs::read(&elf_path).unwrap_or_else(|e| {
        panic!(
            "Failed to read vote-circuit ELF at {}: {}\n\
             Hint: build the guest methods first with `cargo build -p multisig-methods`",
            elf_path.display(),
            e
        )
    });

    let image_id = risc0_binfmt::compute_image_id(&elf_bytes)
        .expect("Failed to compute image ID from vote-circuit ELF");

    let words = image_id.as_words();

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let dest_path = out_dir.join("image_id.rs");
    let mut f = fs::File::create(&dest_path).unwrap();
    writeln!(
        f,
        "/// Auto-generated from vote-circuit ELF. Do not edit."
    )
    .unwrap();
    writeln!(
        f,
        "const VOTE_CIRCUIT_IMAGE_ID: [u32; 8] = {words:?};"
    )
    .unwrap();
}
