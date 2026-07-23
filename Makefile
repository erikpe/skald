MSRV := $(shell sed -n 's/^rust-version = "\(.*\)"/\1/p' Cargo.toml)

.PHONY: help fmt runtime fmt-check build-check lint docs-check static-check \
	compiler-test cli-test docs-test golden-expectations-test golden-run-test \
	golden-test runtime-test test msrv-check robustness-long check

help:
	@echo "Skald repository commands:"
	@echo ""
	@echo "Developer commands and shared build prerequisites:"
	@echo "  make fmt              Format Rust sources"
	@echo "  make runtime          Build the C runtime archive"
	@echo ""
	@echo "Ordinary static validation:"
	@echo "  make static-check     Run formatting, build, lint, and documentation checks"
	@echo "  make fmt-check        Check Rust formatting"
	@echo "  make build-check      Type-check every Rust target"
	@echo "  make lint             Run Clippy for the workspace"
	@echo "  make docs-check       Check repository-local documentation links and indexes"
	@echo ""
	@echo "Ordinary behavioral test suites:"
	@echo "  make test             Run all ordinary behavioral test suites"
	@echo "  make compiler-test    Run all skald-compiler tests"
	@echo "  make cli-test         Run skac binary and CLI tests"
	@echo "  make docs-test        Run skald-docs-check unit and documentation tests"
	@echo "  make golden-test      Run golden expectation and end-to-end tests"
	@echo "  make golden-expectations-test Run golden sidecar and mismatch-reporting tests"
	@echo "  make golden-run-test  Run source-to-executable and compile-failure goldens"
	@echo "  make runtime-test     Build and run C runtime tests"
	@echo ""
	@echo "Extended validation:"
	@echo "  make msrv-check       Type-check every Rust target with the declared MSRV"
	@echo "  make robustness-long  Run extended deterministic frontend robustness tests"
	@echo ""
	@echo "Complete ordinary validation:"
	@echo "  make check            Run the complete repository validation suite"

# Developer commands and shared build prerequisites.
fmt:
	cargo fmt --all

runtime:
	$(MAKE) -C runtime

# Ordinary static validation included in static-check.
static-check: fmt-check build-check lint docs-check

fmt-check:
	cargo fmt --all -- --check

build-check:
	cargo check --locked --workspace --all-targets

lint:
	cargo clippy --locked --workspace --all-targets -- -D warnings

docs-check:
	cargo run --quiet --locked -p skald-docs-check -- .

# Ordinary behavioral suites included in test.
test: compiler-test cli-test golden-test runtime-test docs-test

compiler-test:
	cargo test --locked -p skald-compiler

cli-test:
	cargo test --locked -p skac --bin skac --test cli

docs-test:
	cargo test --locked -p skald-docs-check

golden-expectations-test:
	cargo test --locked -p skac --test golden-expectations

golden-run-test: runtime
	cargo test --locked -p skac --test golden

golden-test: golden-expectations-test golden-run-test

runtime-test: runtime
	$(MAKE) -C runtime test

# Extended validation for less frequent external runs.
msrv-check:
	@test -n "$(MSRV)" || { echo "could not read workspace.package.rust-version from Cargo.toml" >&2; exit 1; }
	cargo +$(MSRV) check --locked --workspace --all-targets

robustness-long:
	SKALD_ROBUSTNESS_CASES=10000 cargo test --locked -p skald-compiler --test generative_robustness

# Complete ordinary validation gate.
check: static-check test
