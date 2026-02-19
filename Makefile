# Treasury Program — Quick Commands
#
# Prerequisites:
#   - Rust + risc0 toolchain installed
#   - wallet CLI installed (`cargo install --path wallet` from lssa repo)
#   - Sequencer running locally
#   - wallet setup done (`wallet setup`)
#
# Quick start:
#   make build deploy setup create-vault
#   make send RECIPIENT=<account_id> AMOUNT=100
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

# Load saved state if it exists (file is KEY=VALUE format, directly includable)
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

.PHONY: help build deploy setup create-vault send deposit status clean

help: ## Show this help
	@echo "Treasury Program — Make Targets"
	@echo ""
	@echo "  make build                 Build the guest binary (needs risc0 toolchain)"
	@echo "  make deploy                Deploy multisig + token programs to sequencer"
	@echo "  make setup                 Create accounts needed (token_def + signer)"
	@echo "  make create-vault          Create a vault (mint tokens into multisig PDA)"
	@echo "  make send                  Send tokens from vault (RECIPIENT=<id> AMOUNT=<n>)"
	@echo "  make deposit               Deposit tokens into vault (SENDER=<id> AMOUNT=<n>)"
	@echo "  make status                Show saved state (account IDs, etc.)"
	@echo "  make clean                 Remove saved state"
	@echo ""
	@echo "Required env: LSSA_DIR=<path to lssa repo>"
	@echo ""
	@echo "Example full flow:"
	@echo "  export LSSA_DIR=../lssa"
	@echo "  make build deploy setup create-vault"
	@echo "  make send RECIPIENT=\$$(wallet account new public | grep -oP '[A-Za-z0-9]{32,}') AMOUNT=100"

build: ## Build the multisig guest binary
	cargo risczero build --manifest-path methods/guest/Cargo.toml
	@echo ""
	@echo "✅ Guest binary built: $(MULTISIG_BIN)"
	@ls -la $(MULTISIG_BIN)

deploy: ## Deploy multisig and token programs to sequencer
	@test -f "$(MULTISIG_BIN)" || (echo "ERROR: Treasury binary not found. Run 'make build' first."; exit 1)
	@test -f "$(TOKEN_BIN)" || (echo "ERROR: Token binary not found at $(TOKEN_BIN). Set LSSA_DIR correctly."; exit 1)
	wallet deploy-program $(MULTISIG_BIN)
	wallet deploy-program $(TOKEN_BIN)
	@echo ""
	@echo "✅ Programs deployed"

setup: ## Create accounts needed for multisig operations
	@echo "Creating token definition account..."
	$(eval TOKEN_DEF_ID := $(shell wallet account new public 2>&1 | sed -n 's/.*Public\/\([A-Za-z0-9]*\).*/\1/p'))
	@echo "Token definition: $(TOKEN_DEF_ID)"
	$(call save_var,TOKEN_DEF_ID,$(TOKEN_DEF_ID))
	@echo ""
	@echo "Creating signer account (authorized to send from vault)..."
	$(eval SIGNER_ID := $(shell wallet account new public 2>&1 | sed -n 's/.*Public\/\([A-Za-z0-9]*\).*/\1/p'))
	@echo "Signer: $(SIGNER_ID)"
	$(call save_var,SIGNER_ID,$(SIGNER_ID))
	@echo ""
	@echo "✅ Accounts created and saved to $(STATE_FILE)"
	@echo "   TOKEN_DEF_ID=$(TOKEN_DEF_ID)"
	@echo "   SIGNER_ID=$(SIGNER_ID)"

create-vault: ## Create a vault (mints tokens into multisig PDA). SIGNERS="id1 id2" for multiple.
	$(call require_state,TOKEN_DEF_ID)
	$(call require_state,SIGNER_ID)
	@test -f "$(MULTISIG_BIN)" || (echo "ERROR: Treasury binary not found. Run 'make build' first."; exit 1)
	@test -f "$(TOKEN_BIN)" || (echo "ERROR: Token binary not found. Set LSSA_DIR correctly."; exit 1)
	cd examples/program_deployment && cargo run --bin deploy_and_create_vault -- \
		../../$(MULTISIG_BIN) \
		$(TOKEN_BIN) \
		$(TOKEN_DEF_ID) \
		$(SIGNER_ID) $(EXTRA_SIGNERS)

send: ## Send tokens from vault (RECIPIENT=<id> AMOUNT=<n>)
	@if [ -z "$(RECIPIENT)" ]; then echo "Usage: make send RECIPIENT=<account_id> AMOUNT=<n>"; exit 1; fi
	@if [ -z "$(AMOUNT)" ]; then echo "Usage: make send RECIPIENT=<account_id> AMOUNT=<n>"; exit 1; fi
	$(call require_state,TOKEN_DEF_ID)
	$(call require_state,SIGNER_ID)
	@test -f "$(MULTISIG_BIN)" || (echo "ERROR: Treasury binary not found."; exit 1)
	@test -f "$(TOKEN_BIN)" || (echo "ERROR: Token binary not found."; exit 1)
	cd examples/program_deployment && cargo run --bin send_from_vault -- \
		../../$(MULTISIG_BIN) \
		$(TOKEN_BIN) \
		$(TOKEN_DEF_ID) \
		$(RECIPIENT) \
		$(AMOUNT) \
		$(SIGNER_ID)

status: ## Show saved state
	@echo "Treasury State (from $(STATE_FILE)):"
	@echo "──────────────────────────────────────"
	@if [ -f "$(STATE_FILE)" ]; then cat $(STATE_FILE); else echo "(no state saved — run 'make setup')"; fi
	@echo ""
	@echo "Binaries:"
	@ls -la $(MULTISIG_BIN) 2>/dev/null || echo "  multisig.bin: NOT BUILT (run 'make build')"
	@ls -la $(TOKEN_BIN) 2>/dev/null || echo "  token.bin: NOT FOUND (check LSSA_DIR)"

clean: ## Remove saved state
	rm -f $(STATE_FILE) $(STATE_FILE).tmp
	@echo "✅ State cleaned"
