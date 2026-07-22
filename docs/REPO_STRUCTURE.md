# Test Migration Guide

Status: legacy migration input. Durable compiler architecture is authoritative
in [Compiler Architecture](compiler/README.md), and target-independent phase
contracts are authoritative in
[Compiler Phases and Intermediate Representations](compiler/PHASES_AND_IR.md).
This document temporarily owns implementation areas that do not yet have
focused compiler and development guides.

## Runtime

The focused [runtime ABI](compiler/RUNTIME_ABI.md) now owns the public C
surface, version and link guard, platform requirements, output records,
failure behavior, and responsibility boundary. This compatibility heading
remains only so links created before the documentation migration have a useful
destination.

## x86-64 System V backend

The focused [backend and target contract](compiler/BACKEND.md) now owns target
selection, legality, data layout, calling conventions, frames, instruction
selection, symbols, and assembly emission. This compatibility heading remains
only so links created before the documentation migration have a useful
destination.

## Driver and artifacts

The focused [driver and artifacts](compiler/DRIVER_AND_ARTIFACTS.md) guide now
owns compiler orchestration, CLI modes, toolchain and runtime selection, input
protection, artifact publication, and structured failures. This compatibility
heading remains only so links created before the documentation migration have
a useful destination.

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

Contributor prerequisites, supported toolchains, Makefile entry points, and
external clean-checkout automation are defined by the
[development workflow](development/README.md).

## Debugging

The current [debugging artifacts guide](DEBUGGING.md) owns detailed renderer,
dump, verifier, and assembly-inspection workflows until its focused
development replacement is created.
