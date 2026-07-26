# Resolved Optional-Values Maintainability Discoveries

Status: resolved.

The optional implementation remains centralized behind the existing syntax,
type-checking, HIR, MIR, verifier, and backend facades. This resolved follow-up
improved internal verifier ownership without changing optional semantics,
diagnostics, MIR public paths, or verifier invocation order.

## Split optional MIR verification by analysis responsibility

**Evidence:** `crates/skald-compiler/src/mir/verify/optional.rs` is about 1,400
lines and owns three independently understandable responsibilities:
instruction-local optional structure and failure-edge checks, path-sensitive
definite optional-storage initialization, and path-sensitive checked-view
guard analysis. The file is cohesive at the feature level, but navigating or
changing one analysis currently requires scanning the other two.

**Likely owner:** `crates/skald-compiler/src/mir/verify/optional/`.

**Useful boundary:** Keep a small private `optional/mod.rs` facade with the
existing `Verifier` entry points. Move structural checks, initialized-storage
analysis, and guard analysis into private responsibility-named submodules.
Preserve verifier invocation order, diagnostic order and wording, MIR public
paths, and the existing focused mutation corpus. Do not combine this move with
semantic changes to optional state or guards.

**Resolution:** The private optional verifier now uses a five-line
`optional/mod.rs` facade with responsibility-focused `structural`,
`initialization`, and `guards` submodules. Existing `Verifier` entry points
remain crate-private, their invocation order is unchanged, and the structural
mutation corpus plus focused initialization and guard tests continue to pass
without diagnostic changes.
