# Multisig Program — Quick Commands
#
# Prerequisites:
#   - Rust + risc0 toolchain installed
#   - wallet CLI installed (`cargo install --path wallet` from lssa repo)
#   - Sequencer running locally
#   - wallet setup done (`wallet setup`)
#
# Quick start:
#   make build deploy
#   multisig create --threshold 2 --member <ID1> --member <ID2> --member <ID3>
#
# State is saved in .multisig-state so you don't have to re-enter IDs.

SHELL := /bin/bash
STATE_FILE := .multisig-state
PROGRAMS_DIR := target/riscv32im-risc0-zkvm-elf/docker

# Token program binary — set this to point to your lssa build
# e.g. LSSA_DIR=../lssa
LSSA_DIR ?= $(error Set LSSA_DIR to your lssa repo root, e.g. make build LSSA_DIR=../lssa)
TOKEN_BIN := $(LSSA_DIR)/artifacts/program_methods/token.bin

MULTISIG_BIN := $(PROGRAMS_DIR)/multisig.bin

# ── Helpers ──────────────────────────────────────────────────────────────────

-include $(STATE_FILE)

define save_var
	@grep -v '^$(1)=' $(STATE_FILE) 2>/dev/null > $(STATE_FILE).tmp || true
	@echo '$(1)=$(2)' >> $(STATE_FILE).tmp
	@mv $(STATE_FILE).tmp $(STATE_FILE)
endef

define require_state
	@if [ -z "$($(1))" ]; then echo "ERROR: $(1) not set. Run the required step first or set it manually."; exit 1; fi
endef

# ── Targets ──────────────────────────────────────────────────────────────────

.PHONY: help build build-cli deploy status clean test

help: ## Show this help
	@echo "Multisig Program — Make Targets"
	@echo ""
	@echo "  make build                 Build the guest binary (needs risc0 toolchain)"
	@echo "  make build-cli             Build the standalone multisig CLI"
	@echo "  make deploy                Deploy multisig + token programs to sequencer"
	@echo "  make test                  Run unit tests"
	@echo "  make status                Show saved state (account IDs, etc.)"
	@echo "  make clean                 Remove saved state"
	@echo ""
	@echo "Required env: LSSA_DIR=<path to lssa repo>"

build: ## Build the multisig guest binary
	cargo risczero build --manifest-path methods/guest/Cargo.toml
	@echo ""
	@echo "✅ Guest binary built: $(MULTISIG_BIN)"
	@ls -la $(MULTISIG_BIN)

build-cli: ## Build the standalone multisig CLI
	cargo build --bin multisig -p multisig-cli
	@echo ""
	@echo "✅ CLI built: target/debug/multisig"

deploy: ## Deploy multisig and token programs to sequencer
	@test -f "$(MULTISIG_BIN)" || (echo "ERROR: Multisig binary not found. Run 'make build' first."; exit 1)
	@test -f "$(TOKEN_BIN)" || (echo "ERROR: Token binary not found at $(TOKEN_BIN). Set LSSA_DIR correctly."; exit 1)
	wallet deploy-program $(MULTISIG_BIN)
	wallet deploy-program $(TOKEN_BIN)
	@echo ""
	@echo "✅ Programs deployed"

test: ## Run unit tests
	cargo test -p multisig_program

status: ## Show saved state
	@echo "Multisig State (from $(STATE_FILE)):"
	@echo "──────────────────────────────────────"
	@if [ -f "$(STATE_FILE)" ]; then cat $(STATE_FILE); else echo "(no state saved)"; fi
	@echo ""
	@echo "Binaries:"
	@ls -la $(MULTISIG_BIN) 2>/dev/null || echo "  multisig.bin: NOT BUILT (run 'make build')"
	@ls -la $(TOKEN_BIN) 2>/dev/null || echo "  token.bin: NOT FOUND (check LSSA_DIR)"

clean: ## Remove saved state
	rm -f $(STATE_FILE) $(STATE_FILE).tmp
	@echo "✅ State cleaned"
