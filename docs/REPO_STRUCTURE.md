# Backend, Runtime, Driver, and Test Migration Guide

Status: legacy migration input. Durable compiler architecture is authoritative
in [Compiler Architecture](compiler/README.md), and target-independent phase
contracts are authoritative in
[Compiler Phases and Intermediate Representations](compiler/PHASES_AND_IR.md).
This document temporarily owns implementation areas that do not yet have
focused compiler and development guides.

## Runtime

The C11 runtime builds `libskald_runtime.a` and exposes a versioned ABI. Its
version-4 archive defines the no-op link marker `ska_rt_abi_v4`. Every backend
must reference the current marker from its generated process entry wrapper;
the linker therefore rejects an archive for a different ABI before producing
an executable. Incompatible runtime revisions must change both the marker
symbol and the compiler reference. The separate `ska_rt_abi_version()` query
is retained for inspection and direct runtime tests, not link compatibility.

The current bootstrap output operations are:

```text
ska_rt_println_i64
ska_rt_println_u64
ska_rt_println_u8
ska_rt_println_f64_bits
ska_rt_println_bool
```

Integer operations print locale-independent decimal records; boolean output is
lowercase; floating output exposes exact binary64 bits. Every successful
record ends with LF and is flushed. A detected write or flush failure
terminates the process unsuccessfully.

Future runtime responsibilities may include allocation, reference counting,
panic support, and dynamic type metadata. Garbage collection, tracing roots,
safepoints, and write barriers do not belong here.

## x86-64 System V backend

The backend separates target legality, primitive and class data layout, System
V argument/result classification, frame layout, MIR-to-machine lowering, typed
assembly representation, GNU assembly emission, and identity-derived symbols.

At the supported external C boundary, Skald `i64`, `u64`, `u8`, `f64`, `bool`,
and `unit` are realized as compatible C `int64_t`, `uint64_t`, `uint8_t`,
`double`, `bool` (`_Bool`), and `void` respectively. The runtime requires
eight-bit bytes and IEEE-754 binary64-compatible C `double`.

Class fields are laid out in declaration order with checked size/alignment
arithmetic. `i64`, `u64`, and `f64` use 8-byte size/alignment; `u8` and `bool`
use 1 byte. Empty classes remain addressable with size/alignment one. Object
locals receive aligned contiguous frame storage, and projected places are
resolved to addresses only in the backend.

Initializers and methods receive the object address as a hidden first integer-
class argument. Integer and SSE arguments use independent register sequences;
overflow arguments share source-ordered stack slots. The current lowering is
intentionally stack-heavy and can later be replaced by register allocation
without changing MIR.

Destructors use the same hidden-receiver call path. Recursive cleanup projects
the existing object place through target-owned field offsets and emits no
aggregate copies, allocation, or deallocation. Empty destruction plans produce
no instructions; user bodies and nested class fields execute in the exact
order already verified in MIR.

The internal alias ABI represents each alias as one integer-class pointer in
source parameter order. `ref` and `mut ref` have identical machine
representations. Internal class value arguments are caller-reserved object
homes passed as one integer-class address, and object results use one hidden
destination address. These are Skald-internal conventions, not a public object
ABI.

Internal symbols derive from stable identities. External declarations retain
their exact source symbol. The generated C-compatible `main` wrapper calls the
ID-selected Skald entry function and exposes its low result bits as the Linux
process status.

## Driver and artifacts

`skac` supports executable output and `--emit asm`. Executable mode streams
assembly to the configured C compiler driver and links the runtime archive. It
does not construct a shell command.

Assembly and executable publication use same-directory temporary files and one
final rename. Failures preserve existing output and clean unpublished
temporaries through RAII. Output paths that alias the input through the same
path, a symbolic link, or a hard link are rejected.

## Testing

The repository uses complementary layers:

1. colocated Rust unit tests for phase behavior and invariants;
2. crate integration tests for public API and cross-phase behavior;
3. exact AST, resolved, HIR, MIR, and assembly dumps;
4. C runtime contract, successful-output, and fatal-output tests; and
5. golden source programs for native behavior and exact diagnostics.

Deterministic hostile frontend inputs and MIR mutations supplement these
layers. Retained non-Rust inputs live under `tests/compiler/robustness/`; Rust
harnesses remain beside their owning phase or in the compiler crate's
integration-test directory.

Golden programs are compiled in independent processes to compare assembly or
diagnostics byte-for-byte. Native cases separately check stdout, empty stderr,
and process status. The object-determinism integration test compares AST,
resolved IR, HIR, MIR, and assembly across independent processes.

`make help` is the authoritative command inventory. `make check` is the
complete repository gate, and external infrastructure runs it regularly from
clean checkouts.

## Debugging

The current [debugging artifacts guide](DEBUGGING.md) owns detailed renderer,
dump, verifier, and assembly-inspection workflows until DOC15 creates its
focused development replacement.
