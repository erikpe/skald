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

Test layers, placement, fixtures, determinism, robustness, and focused
commands are now defined by [Testing](development/TESTING.md). Contributor
prerequisites and repository gates remain in the
[development workflow](development/README.md). This compatibility heading
remains only until the migration document is removed.

## Debugging

[Debugging the Compiler](development/DEBUGGING.md) owns renderer, dump,
verifier, assembly-inspection, and symptom-to-owner workflows. The old
[`DEBUGGING.md`](DEBUGGING.md) path is a temporary compatibility entry point.
