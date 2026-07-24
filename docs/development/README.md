# Development Workflow

Status: authoritative for contributor prerequisites, Makefile entry points,
supported Rust toolchains, and local/external validation. See
[Testing](TESTING.md) for test ownership and selection and
[Debugging the Compiler](DEBUGGING.md) for inspection workflows.

## Prerequisites

Skald development currently requires:

- Linux;
- rustup and the repository-selected stable Rust toolchain;
- the rustfmt and Clippy components selected by `rust-toolchain.toml`;
- GNU Make;
- a C11 compiler driver; and
- a static archiver.

The Rust workspace has no third-party crate dependencies. Native compilation
and runtime tests require the host C tools even when a change touches only Rust
code.

## Makefile interface

The repository-root Makefile is the shared interface for contributor and
automated validation. Run:

```text
make help
```

for the current detailed command inventory. Focused targets are useful while
iterating; `make check` is the complete ordinary gate for the selected stable
toolchain. Its dependency graph is explicit: `static-check` combines
formatting, workspace checks, Clippy, and documentation validation, while
`test` combines compiler, CLI, golden, direct C runtime, and documentation
tests. The bounded robustness cases are part of the compiler suite; only the
larger scheduled robustness run and the minimum-supported-Rust check remain
outside the ordinary gate for less frequent external validation.

Commands should remain independently runnable through the Makefile. A helper
script may implement a repeated workflow, but it must not become the only way
to invoke a compiler or validation responsibility.

## Minimum supported Rust version

`Cargo.toml` is authoritative for `workspace.package.rust-version`, currently
Rust 1.82.0. The repository toolchain file intentionally selects stable for
ordinary work; minimum-version compilation is a separate local target:

```text
rustup toolchain install 1.82.0 --profile minimal
make msrv-check
```

`make msrv-check` reads the declared version from the workspace manifest and
checks every workspace target with the lockfile. It does not install a
toolchain or mutate contributor configuration. Run it whenever manifests,
supported Rust syntax, or the toolchain contract changes; it is also part of
release or roadmap closeout when requested.

## Change validation

During implementation, run the focused test or check that owns the changed
behavior. Before handing off a completed change, run `make check` and
`git diff --check`. Add `make msrv-check` under the conditions above. The
[driver guide](../compiler/DRIVER_AND_ARTIFACTS.md) documents compiler/toolchain
selection and artifact behavior; `make help` remains the command inventory.

Cargo writes under `target/`, and repository runtime, golden, and compiler
artifacts write under `build/`. Both directories are ignored and must not be
treated as source inputs or committed results.

## External automation

Existing external infrastructure runs `make check` regularly from clean
checkouts. The repository intentionally contains no CI job configuration: the
same Makefile targets are the reproducible local and external automation
boundary. New checks belong in an appropriate Makefile target and, when part
of the ordinary gate, in `make check`; they do not require a second
repository-only workflow.
