# Makefile — GitGov Control Plane Server
# Usage: make <target>
#
# Targets:
#   make check      cargo check + clippy (fast, no linking)
#   make test       cargo test (all unit tests, no server needed)
#   make smoke      run smoke_contract.sh against a running server
#   make chat-bench run chat capacity benchmark (p50/p95/p99, throughput, errors)
#   make all        check + test

.PHONY: check test smoke chat-bench all

SERVER_URL ?= http://127.0.0.1:3000
API_KEY    ?= 57f1ed59-371d-46ef-9fdf-508f59bc4963
CHAT_REQUESTS ?= 120
CHAT_CONCURRENCY ?= 12
CHAT_SCENARIO ?= mixed

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

chat-bench:
	@echo ""
	@echo "Running chat capacity benchmark against $(SERVER_URL)"
	@echo "Requests=$(CHAT_REQUESTS) Concurrency=$(CHAT_CONCURRENCY) Scenario=$(CHAT_SCENARIO)"
	@echo ""
	python tests/chat_capacity_test.py \
		--server-url "$(SERVER_URL)" \
		--api-key "$(API_KEY)" \
		--requests "$(CHAT_REQUESTS)" \
		--concurrency "$(CHAT_CONCURRENCY)" \
		--scenario "$(CHAT_SCENARIO)" \
		--out-json "tests/artifacts/chat_capacity_$(CHAT_SCENARIO).json"

all: check test
