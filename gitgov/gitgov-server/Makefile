# Makefile — GitGov Control Plane Server
# Usage: make <target>
#
# Targets:
#   make check      cargo check + clippy (fast, no linking)
#   make test       cargo test (all unit tests, no server needed)
#   make smoke      run smoke_contract.sh against a running server
#   make all        check + test

.PHONY: check test smoke all

SERVER_URL ?= http://127.0.0.1:3000
API_KEY    ?= 57f1ed59-371d-46ef-9fdf-508f59bc4963

check:
	cargo check
	cargo clippy -- -D warnings

test:
	cargo test

smoke:
	@echo ""
	@echo "Running smoke/contract test against $(SERVER_URL)"
	@echo "Requires server to be running. Start with: cargo run"
	@echo ""
	SERVER_URL=$(SERVER_URL) API_KEY=$(API_KEY) bash tests/smoke_contract.sh

all: check test
