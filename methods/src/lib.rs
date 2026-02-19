// This crate exists only to host the risc0 build infrastructure.
// The actual guest binary is in guest/src/bin/multisig.rs.
include!(concat!(env!("OUT_DIR"), "/methods.rs"));
