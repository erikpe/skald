# Development Workflow

Status: authoritative for contributor prerequisites, Makefile entry points,
supported Rust toolchains, and local/external validation. See
[Testing](TESTING.md) for test ownership and selection and
[Debugging the Compiler](DEBUGGING.md) for inspection workflows.
The reproducible enabled-versus-omitted trace measurement is documented in
[Panic Runtime Trace Performance](PANIC_RUNTIME_TRACE_PERFORMANCE.md).
The fused-range acceptance measurement is documented in
[Tight Range-Loop Performance](RANGE_LOOP_PERFORMANCE.md).

## Prerequisites

Skald development currently requires:

- Linux;
- rustup and the repository-selected stable Rust toolchain;
- the rustfmt and Clippy components selected by `rust-toolchain.toml`;
- GNU Make;
- a C11 compiler driver; and
- a static archiver.

The optional performance benchmarks additionally require Python 3; neither
normal builds nor repository validation use Python.

Production compiler crates and the documentation checker have no third-party
crate dependencies. The `skald-golden` repository tool is the narrow
exception: it uses maintained TOML and Serde crates to decode the versioned
golden-test schema and report precise field paths, Serde JSON for the machine
report, plus the narrowly scoped `nix` process/signal API to terminate complete
Linux child process groups without repository-owned unsafe code. JUnit and
human reports are rendered by the tool without another production dependency.
Those dependencies and their complete transitive graph are recorded in
`Cargo.lock`; they do not flow into `skac`, `skald-compiler`, generated
programs, or the runtime. Native compilation and runtime tests require the host
C tools even when a change touches only Rust code.

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
`test` combines compiler, CLI, golden-runner, end-to-end golden, direct C
runtime, and documentation tests. The bounded robustness cases are part of
the compiler suite; only the larger scheduled robustness run and the
minimum-supported-Rust check remain outside the ordinary gate for less
frequent external validation.

Commands should remain independently runnable through the Makefile. A helper
script may implement a repeated workflow, but it must not become the only way
to invoke a compiler or validation responsibility.

`make golden-test` builds `skac` and `skald-golden` and runs the complete suite
in the default determinism-off mode. Use `make golden-filter` or
`make golden-exact` for common focused runs and
`make golden-determinism-test` for the complete repeated-process audit. The
extended `make golden-release-test` target builds both tools with Cargo's
release profile and runs the complete suite once; it is part of `make
check-long`. The
[`scripts/golden.sh`](../../scripts/README.md) convenience wrapper exposes the
runner's full filtering and reporting surface while preserving those Makefile
validation entry points.

`make runtime-trace-benchmark` builds the compiler and runtime, then compares
the default enabled trace policy with complete compile-time omission. It is a
measurement procedure, not a timing gate, and is intentionally excluded from
`make check`.

`make generic-vec-benchmark` similarly measures a representative generic
vector growth, structural-copy, pop, and clear workload. It reports compile,
native-runtime, and artifact-size observations without imposing a timing
threshold or joining `make check`.

`make range-loop-benchmark` compares immediate `u8`, `u64`, and `i64` ranges
with matched handwritten `while` loops and enforces the documented maximum
10% median range overhead. It remains outside `make check`; deterministic MIR,
assembly, and native-result tests are the ordinary correctness gates.

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
the locked dependency graph, supported Rust syntax, or the toolchain contract
changes; it is also part of release or roadmap closeout when requested. In
particular, runner dependency updates must retain Rust-1.82-compatible
transitive releases in `Cargo.lock`.

## Change validation

During implementation, run the focused test or check that owns the changed
behavior. Before handing off a completed change, run `make check` and
`git diff --check`. Add `make msrv-check` under the conditions above. The
[driver guide](../compiler/DRIVER_AND_ARTIFACTS.md) documents compiler/toolchain
selection and artifact behavior; `make help` remains the command inventory.
For an interactive compiler run, `skac -v`, `-vv`, and `-vvv` add phase,
details, and trace reports on stderr without changing artifact or program
output. Use `--diagnostic-level error` independently when warnings would
obscure a focused failure.

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
