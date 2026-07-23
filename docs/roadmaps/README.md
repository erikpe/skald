# Active and Planned Roadmaps

This directory contains implementation roadmaps that are planned or in
progress. Completed roadmaps and resolved discovery records move to
[`../archive/`](../archive/README.md).

## Planned implementation

- [Intel-syntax x86-64 assembly](INTEL_ASSEMBLY_SYNTAX_ROADMAP.md) — planned;
  INTEL0 is next. Permanently switch the existing target's deterministic GNU
  assembly artifact from AT&T notation to Intel syntax with `noprefix` while
  preserving its System V ABI, runtime boundary, and native behavior. It has
  no dependency on another roadmap.

The completed polymorphism profile is now the baseline for focused
shared-ownership work. Shared ownership is the next object-model direction;
checked exceptions follow because they extend cleanup to exceptional control
flow. Both remain exploratory until focused designs and roadmaps make their
contracts reviewable. Other unscheduled language directions and their maturity
are owned by the [status matrix](../language/STATUS.md#not-implemented).
