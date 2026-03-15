# Makefile — GitGov Control Plane Server
# Usage: make <target>
#
# Targets:
#   make check      cargo check + clippy (fast, no linking)
#   make test       cargo test (all unit tests, no server needed)
#   make smoke      run smoke_contract.sh against a running server
#   make governance-smoke run governance v2 multi-session smoke against a running server
#   make chat-bench run chat capacity benchmark (p50/p95/p99, throughput, errors)
#   make capacity-mixed run mixed-profile capacity benchmark (250 concurrent target)
#   make capacity-soak run 60-min soak with mixed profile
#   make all        check + test

.PHONY: check test smoke governance-smoke chat-bench capacity-mixed capacity-soak all

SERVER_URL ?= http://127.0.0.1:3000
API_KEY    ?=
API_KEYS ?=
API_KEYS_FILE ?=
ADMIN_API_KEY ?=
DEV_API_KEY_A ?=
DEV_API_KEY_B ?=
POLICY_REPO ?= acme/repo
ORG_NAME ?= acme
CHAT_REQUESTS ?= 120
CHAT_CONCURRENCY ?= 12
CHAT_SCENARIO ?= mixed
CAPACITY_USERS ?= 250
CAPACITY_DURATION_SEC ?= 1200
CAPACITY_SOAK_DURATION_SEC ?= 3600
CAPACITY_THINK_MS ?= 120
CAPACITY_TIMEOUT_SEC ?= 12
CAPACITY_EVENTS_RATIO ?= 70
CAPACITY_DASHBOARD_RATIO ?= 20
CAPACITY_CHAT_RATIO ?= 10
CAPACITY_OUT ?= tests/artifacts/capacity_mixed_250.json
CAPACITY_SOAK_OUT ?= tests/artifacts/capacity_mixed_250_soak.json

check:
	cargo check
	cargo clippy -- -D warnings

test:
	cargo test

smoke:
	@if [ -z "$(API_KEY)" ]; then echo "API_KEY is required (export API_KEY=... or pass API_KEY=...)"; exit 1; fi
	@echo ""
	@echo "Running smoke/contract test against $(SERVER_URL)"
	@echo "Requires server to be running. Start with: cargo run"
	@echo ""
	SERVER_URL=$(SERVER_URL) API_KEY=$(API_KEY) bash tests/smoke_contract.sh

governance-smoke:
	@if [ -z "$(ADMIN_API_KEY)" ]; then echo "ADMIN_API_KEY is required"; exit 1; fi
	@if [ -z "$(DEV_API_KEY_A)" ]; then echo "DEV_API_KEY_A is required"; exit 1; fi
	@if [ -z "$(DEV_API_KEY_B)" ]; then echo "DEV_API_KEY_B is required"; exit 1; fi
	@echo ""
	@echo "Running governance multi-session smoke against $(SERVER_URL)"
	@echo "Repo=$(POLICY_REPO)"
	@echo "Requires server to be running. Start with: cargo run"
	@echo ""
	SERVER_URL=$(SERVER_URL) \
	ADMIN_API_KEY=$(ADMIN_API_KEY) \
	DEV_API_KEY_A=$(DEV_API_KEY_A) \
	DEV_API_KEY_B=$(DEV_API_KEY_B) \
	POLICY_REPO=$(POLICY_REPO) \
	bash tests/policy_requests_multisession_smoke.sh

chat-bench:
	@if [ -z "$(API_KEY)" ]; then echo "API_KEY is required (export API_KEY=... or pass API_KEY=...)"; exit 1; fi
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

capacity-mixed:
	@if [ -z "$(API_KEYS)" ] && [ -z "$(API_KEYS_FILE)" ] && [ -z "$(API_KEY)" ]; then echo "API_KEYS/API_KEYS_FILE/API_KEY is required"; exit 1; fi
	@echo ""
	@echo "Running mixed capacity benchmark against $(SERVER_URL)"
	@echo "Users=$(CAPACITY_USERS) Duration=$(CAPACITY_DURATION_SEC)s Profile=$(CAPACITY_EVENTS_RATIO)/$(CAPACITY_DASHBOARD_RATIO)/$(CAPACITY_CHAT_RATIO)"
	@echo ""
	@keys="$(API_KEYS)"; \
	if [ -z "$$keys" ] && [ -n "$(API_KEY)" ]; then keys="$(API_KEY)"; fi; \
	python tests/capacity_mixed_profile.py \
		--server-url "$(SERVER_URL)" \
		--api-keys "$$keys" \
		--api-keys-file "$(API_KEYS_FILE)" \
		--org-name "$(ORG_NAME)" \
		--users "$(CAPACITY_USERS)" \
		--duration-sec "$(CAPACITY_DURATION_SEC)" \
		--think-ms "$(CAPACITY_THINK_MS)" \
		--timeout-sec "$(CAPACITY_TIMEOUT_SEC)" \
		--events-ratio "$(CAPACITY_EVENTS_RATIO)" \
		--dashboard-ratio "$(CAPACITY_DASHBOARD_RATIO)" \
		--chat-ratio "$(CAPACITY_CHAT_RATIO)" \
		--out-json "$(CAPACITY_OUT)"

capacity-soak:
	@if [ -z "$(API_KEYS)" ] && [ -z "$(API_KEYS_FILE)" ] && [ -z "$(API_KEY)" ]; then echo "API_KEYS/API_KEYS_FILE/API_KEY is required"; exit 1; fi
	@echo ""
	@echo "Running mixed capacity SOAK against $(SERVER_URL)"
	@echo "Users=$(CAPACITY_USERS) Duration=$(CAPACITY_SOAK_DURATION_SEC)s Profile=$(CAPACITY_EVENTS_RATIO)/$(CAPACITY_DASHBOARD_RATIO)/$(CAPACITY_CHAT_RATIO)"
	@echo ""
	@keys="$(API_KEYS)"; \
	if [ -z "$$keys" ] && [ -n "$(API_KEY)" ]; then keys="$(API_KEY)"; fi; \
	python tests/capacity_mixed_profile.py \
		--server-url "$(SERVER_URL)" \
		--api-keys "$$keys" \
		--api-keys-file "$(API_KEYS_FILE)" \
		--org-name "$(ORG_NAME)" \
		--users "$(CAPACITY_USERS)" \
		--duration-sec "$(CAPACITY_SOAK_DURATION_SEC)" \
		--think-ms "$(CAPACITY_THINK_MS)" \
		--timeout-sec "$(CAPACITY_TIMEOUT_SEC)" \
		--events-ratio "$(CAPACITY_EVENTS_RATIO)" \
		--dashboard-ratio "$(CAPACITY_DASHBOARD_RATIO)" \
		--chat-ratio "$(CAPACITY_CHAT_RATIO)" \
		--out-json "$(CAPACITY_SOAK_OUT)"

all: check test
