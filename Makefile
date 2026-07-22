MSRV := $(shell sed -n 's/^rust-version = "\(.*\)"/\1/p' Cargo.toml)

.PHONY: help fmt fmt-check build-check msrv-check lint compiler-test golden-test robustness-smoke robustness-long runtime runtime-test docs-check check

help:
	@echo "Skald repository commands:"
	@echo "  make fmt              Format Rust sources"
	@echo "  make fmt-check        Check Rust formatting"
	@echo "  make build-check      Type-check every Rust target"
	@echo "  make msrv-check       Type-check every Rust target with the declared MSRV"
	@echo "  make lint             Run Clippy for the workspace"
	@echo "  make compiler-test    Run Rust compiler tests"
	@echo "  make golden-test      Run native source-to-executable golden tests"
	@echo "  make robustness-smoke Run bounded deterministic frontend and MIR robustness tests"
	@echo "  make robustness-long  Run the longer deterministic frontend robustness corpus"
	@echo "  make runtime          Build the C runtime archive"
	@echo "  make runtime-test     Build and run C runtime tests"
	@echo "  make docs-check       Check repository-local documentation links and indexes"
	@echo "  make check            Run the complete repository validation suite"

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

build-check:
	cargo check --locked --workspace --all-targets

msrv-check:
	@test -n "$(MSRV)" || { echo "could not read workspace.package.rust-version from Cargo.toml" >&2; exit 1; }
	cargo +$(MSRV) check --locked --workspace --all-targets

lint:
	cargo clippy --locked --workspace --all-targets -- -D warnings

compiler-test:
	cargo test --locked --workspace

golden-test: runtime
	cargo test --locked -p skac --test golden

robustness-smoke:
	cargo test --locked -p skald-compiler --test generative_robustness
	cargo test --locked -p skald-compiler --lib mir::tests::robustness

robustness-long:
	SKALD_ROBUSTNESS_CASES=10000 $(MAKE) robustness-smoke

runtime:
	$(MAKE) -C runtime

runtime-test:
	$(MAKE) -C runtime test

docs-check:
	cargo run --quiet --locked -p skald-docs-check -- .

check: fmt-check build-check lint compiler-test runtime-test docs-check
