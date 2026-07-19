.PHONY: help fmt fmt-check build-check lint compiler-test golden-test runtime runtime-test check

help:
	@echo "Skald repository commands:"
	@echo "  make fmt           Format Rust sources"
	@echo "  make fmt-check     Check Rust formatting"
	@echo "  make build-check   Type-check every Rust target"
	@echo "  make lint          Run Clippy for the workspace"
	@echo "  make compiler-test Run Rust compiler tests"
	@echo "  make golden-test   Run native source-to-executable golden tests"
	@echo "  make runtime       Build the C runtime archive"
	@echo "  make runtime-test  Build and run C runtime tests"
	@echo "  make check         Run the complete repository validation suite"

fmt:
	cargo fmt --all

fmt-check:
	cargo fmt --all -- --check

build-check:
	cargo check --workspace --all-targets

lint:
	cargo clippy --workspace --all-targets -- -D warnings

compiler-test:
	cargo test --workspace

golden-test: runtime
	cargo test -p skac --test golden

runtime:
	$(MAKE) -C runtime

runtime-test:
	$(MAKE) -C runtime test

check: fmt-check build-check lint compiler-test runtime-test
