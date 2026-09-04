MSRV := $(shell sed -n 's/^rust-version = "\(.*\)"/\1/p' Cargo.toml)
GOLDEN_RUNNER := target/debug/skald-golden
GOLDEN_COMPILER := target/debug/skac
GOLDEN_RELEASE_RUNNER := target/release/skald-golden
GOLDEN_RELEASE_COMPILER := target/release/skac

.PHONY: help fmt runtime fmt-check build-check lint docs-check static-check \
	compiler-test cli-test docs-test golden-runner-test mir-measure-test golden-tools \
	golden-release-tools golden-expectations-test golden-test \
	golden-release-test golden-filter golden-exact \
	golden-determinism-test runtime-test runtime-trace-benchmark test \
	generic-vec-benchmark range-loop-benchmark mir-redundancy-measure \
	msrv-check robustness-long check check-long

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
	@echo "  make golden-runner-test Run skald-golden schema and runner-library tests"
	@echo "  make mir-measure-test Run local final-MIR measurement tool tests"
	@echo "  make golden-test      Run all goldens in default determinism-off mode"
	@echo "  make golden-expectations-test Run focused byte, ownership, and report tests"
	@echo "  make golden-filter GOLDEN_FILTER='syntax/**'  Run matching golden leaves"
	@echo "  make golden-exact GOLDEN_ID='calls/functions::direct_call::default::return_value'  Run one leaf"
	@echo "  make runtime-test     Build and run C runtime tests"
	@echo ""
	@echo "Extended validation:"
	@echo "  make golden-release-test Run all goldens with release-built tools"
	@echo "  make golden-determinism-test Run all goldens in full determinism mode"
	@echo "  make runtime-trace-benchmark Compare enabled and omitted panic trace overhead"
	@echo "  make generic-vec-benchmark Measure representative generic Vec growth"
	@echo "  make range-loop-benchmark Compare fused ranges with matched while loops"
	@echo "  make mir-redundancy-measure Measure the reviewed final-MIR redundancy corpus"
	@echo "  make msrv-check       Type-check every Rust target with the declared MSRV"
	@echo "  make robustness-long  Run extended deterministic frontend robustness tests"
	@echo ""
	@echo "Complete validation:"
	@echo "  make check            Run the complete repository validation suite"
	@echo "  make check-long       Run ordinary and extended validation"

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
test: cli-test golden-runner-test mir-measure-test golden-test runtime-test docs-test compiler-test

compiler-test:
	cargo test --locked -p skald-compiler

cli-test:
	cargo test --locked -p skac --bin skac --test cli

docs-test:
	cargo test --locked -p skald-docs-check

golden-runner-test:
	cargo test --locked -p skald-golden

mir-measure-test:
	cargo test --locked -p skald-mir-measure

golden-expectations-test:
	cargo test --locked -p skald-golden --test planning --test process_execution --test reporting

golden-tools:
	cargo build --locked -p skac -p skald-golden

golden-release-tools:
	cargo clippy --locked --release -p skac -p skald-golden -- -D warnings
	cargo build --locked --release -p skac -p skald-golden

golden-test: golden-tools
	$(GOLDEN_RUNNER) --compiler $(GOLDEN_COMPILER) --determinism off

golden-release-test: golden-release-tools
	$(GOLDEN_RELEASE_RUNNER) --compiler $(GOLDEN_RELEASE_COMPILER) --determinism off

golden-filter: golden-tools
	@test -n "$(GOLDEN_FILTER)" || { echo "set GOLDEN_FILTER to a golden glob" >&2; exit 1; }
	$(GOLDEN_RUNNER) --compiler $(GOLDEN_COMPILER) --determinism off --filter '$(GOLDEN_FILTER)'

golden-exact: golden-tools
	@test -n "$(GOLDEN_ID)" || { echo "set GOLDEN_ID to a canonical golden leaf ID" >&2; exit 1; }
	$(GOLDEN_RUNNER) --compiler $(GOLDEN_COMPILER) --determinism off --exact '$(GOLDEN_ID)'

golden-determinism-test: golden-tools
	$(GOLDEN_RUNNER) --compiler $(GOLDEN_COMPILER) --determinism full

runtime-trace-benchmark: runtime
	cargo build --locked -p skac
	python3 scripts/measure_panic_runtime_trace.py --compiler target/debug/skac

generic-vec-benchmark: runtime
	cargo build --locked -p skac
	python3 scripts/measure_generic_vec.py --compiler target/debug/skac

range-loop-benchmark: runtime
	cargo build --locked -p skac
	python3 scripts/measure_range_loops.py --compiler target/debug/skac --require-target

mir-redundancy-measure:
	cargo run --quiet --locked -p skald-mir-measure -- \
		--format json --output build/measurements/local-mir-redundancy.json

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

# Complete ordinary and extended validation gate.
check-long: check golden-determinism-test golden-release-test runtime-trace-benchmark msrv-check robustness-long generic-vec-benchmark range-loop-benchmark
